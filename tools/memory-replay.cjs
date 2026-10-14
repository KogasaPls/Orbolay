const fs = require('node:fs');
const { spawn, execFileSync } = require('node:child_process');
const root = process.env.ORBOLAY_TEST_ROOT || '/tmp/orbolay-memory-candidate';
const binary = process.env.ORBOLAY_TEST_BINARY;
if (!binary || !fs.existsSync(binary)) throw Error('ORBOLAY_TEST_BINARY must select an existing executable');
const port = 45998;
const plan = { binary, scope: 'Private Xvfb display, separate config/runtime/cache and bridge port', port,
    participants: 12, updateIntervalMs: 200, seconds: 40, rssLimitMiB: 700,
    avatars: 'Empty synthetic avatar fields; no participant images or call data', input: 'No global keybind', cleanup: 'Kill only the detached test process group' };
if (process.argv.includes('--always')) plan.memberDisplay = 'alwaysSemiTransparent';
if (process.argv.includes('--simple-template')) plan.template = 'Minimal text-only HTML';
if (process.argv.includes('--always') || process.argv.includes('--simple-template')) plan.seconds = 20;
if (process.env.ORBOLAY_TEST_SECONDS) plan.seconds = Number(process.env.ORBOLAY_TEST_SECONDS);
if (!Number.isInteger(plan.seconds) || plan.seconds < 10) throw Error('Test duration must be at least 10 seconds');
if (process.argv.includes('--dry-run')) { console.log(JSON.stringify(plan, null, 2)); process.exit(0); }
for (const path of [root, root + '/config/orbolay', root + '/runtime', root + '/cache']) fs.mkdirSync(path, { recursive: true, mode: 0o700 });
const configPath = require('node:path').join(process.env.HOME, '.config/orbolay/config.json');
const config = fs.existsSync(configPath) ? JSON.parse(fs.readFileSync(configPath, 'utf8')) : {};
Object.assign(config, { port, displayName: null, displayIdx: 0, overlayKeybind: null,
    isKeybindEnabled: false, softwareRendering: true, enableMessageNotifications: false,
    userOffsetX: 0, userOffsetY: 0, userAlignment: 'top-left' });
config.userTemplate = null;
config.displayVoiceMembers = 'whenSpeaking';
if (plan.memberDisplay) config.displayVoiceMembers = plan.memberDisplay;
if (plan.template) {
    const directory = root + '/config/orbolay/templates/users';
    fs.mkdirSync(directory, { recursive: true });
    fs.writeFileSync(directory + '/memory-check.html', '<!doctype html><html><head><style>body{margin:0;color:white;font-family:sans-serif;font-size:16px}.speaking{color:green}</style></head><body><div id="user">{{name}}</div></body></html>');
    config.userTemplate = 'memory-check';
}
fs.writeFileSync(root + '/config/orbolay/config.json', JSON.stringify(config));
const env = { ...process.env, XDG_CONFIG_HOME: root + '/config', XDG_CACHE_HOME: root + '/cache', XDG_RUNTIME_DIR: root + '/runtime' };
const log = fs.openSync(root + '/process.log', 'w');
let group, socket;
const samples = [];
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
function overlayPid() {
    try { return Number(execFileSync('pgrep', ['-g', String(group.pid), '-x', 'orbolay'], { encoding: 'utf8' }).trim().split('\n')[0]); }
    catch { return null; }
}
function sample(seconds) {
    const pid = overlayPid();
    if (!pid) throw Error('The isolated overlay exited');
    const status = fs.readFileSync(`/proc/${pid}/status`, 'utf8');
    const rssMiB = Number(status.match(/^VmRSS:\s+(\d+)/m)?.[1] || 0) / 1024;
    const threads = Number(status.match(/^Threads:\s+(\d+)/m)?.[1] || 0);
    const row = { seconds, rssMiB: Math.round(rssMiB * 10) / 10, threads };
    samples.push(row); console.log(JSON.stringify(row));
    if (rssMiB > plan.rssLimitMiB) throw Error('Reached the test memory cap; stopped safely');
}
const users = Array.from({ length: plan.participants }, (_, index) => ({
    userId: `fixture-${index}`, username: `Synthetic speaker ${index}`, avatarUrl: '',
    channelId: 'fixture-room', mute: false, deaf: false, speaking: false, streaming: false, camera: false
}));
function send(cmd, data) { socket.send(JSON.stringify({ cmd, ...data })); }
(async () => {
    try {
        group = spawn('xvfb-run', ['-a', '-s', '-screen 0 1280x720x24 -nolisten tcp', binary, '--websocket', '--port', String(port), '--debug'], { env, detached: true, stdio: ['ignore', log, log] });
        for (let attempt = 0; attempt < 50; attempt++) {
            try {
                const candidate = new WebSocket(`ws://127.0.0.1:${port}`);
                await new Promise((resolve, reject) => { candidate.addEventListener('open', resolve, { once: true }); candidate.addEventListener('error', reject, { once: true }); });
                socket = candidate; break;
            } catch { await sleep(100); }
        }
        if (!socket) throw Error('Isolated overlay bridge did not start');
        send('REGISTER_CONFIG', { userId: users[0].userId, port });
        send('CHANNEL_JOINED', { states: users });
        await sleep(1000);
        if (/Failed to handle websocket message|panicked at/.test(fs.readFileSync(root + '/process.log', 'utf8'))) throw Error('Synthetic payload was not accepted by the actual bridge handler');
        sample(0);
        if (process.env.ORBOLAY_TEST_SCREENSHOT) {
            const pid = overlayPid();
            const display = fs.readFileSync(`/proc/${pid}/environ`, 'utf8').split('\0').find(item => item.startsWith('DISPLAY=')).slice(8);
            const tree = execFileSync('xwininfo', ['-display', display, '-root', '-tree'], { encoding: 'utf8' });
            const windowId = tree.match(/(0x[0-9a-f]+) \"[Oo]rbolay\"/)?.[1];
            if (!windowId) throw Error('Private Orbolay window not found');
            execFileSync('import', ['-display', display, '-window', windowId, process.env.ORBOLAY_TEST_SCREENSHOT]);
        }
        const start = Date.now();
        for (let tick = 0; Date.now() - start < plan.seconds * 1000; tick++) {
            const index = Math.floor(tick / 2) % users.length;
            const user = { ...users[index], speaking: tick % 2 === 0 };
            send('VOICE_STATE_UPDATE', { state: user });
            await sleep(plan.updateIntervalMs);
            if (tick % 25 === 24) sample(Math.round((Date.now() - start) / 1000));
        }
        sample(plan.seconds);
        const warmupRss = samples.find(row => row.seconds >= 10).rssMiB;
        const activeGrowthMiB = Math.max(...samples.filter(row => row.seconds >= 10).map(row => row.rssMiB)) - warmupRss;
        send('CHANNEL_LEFT', {}); await sleep(3000); sample(plan.seconds + 3);
        if (/Failed to handle websocket message|panicked at/.test(fs.readFileSync(root + '/process.log', 'utf8'))) throw Error('An actual bridge handler rejected a synthetic update');
        const result = { plan, samples,
            passed: activeGrowthMiB < 30,
            activeGrowthMiB: Math.round(activeGrowthMiB * 10) / 10,
            rssGrowthMiB: Math.round((samples.at(-1).rssMiB - samples[0].rssMiB) * 10) / 10,
            limitation: 'Short software-rendered run without real avatars cannot exclude slow, image-related or GPU-related retention.' };
        fs.writeFileSync(root + '/result.json', JSON.stringify(result, null, 2));
        if (!result.passed) throw Error('Memory regression: RSS grew by more than 30 MiB after warmup');
    } finally {
        socket?.close();
        if (group?.pid) try { process.kill(-group.pid, 'SIGTERM'); } catch {}
        fs.closeSync(log);
    }
})().catch(error => {
    fs.writeFileSync(root + '/result.json', JSON.stringify({ plan, samples, error: error.message }, null, 2));
    console.error(error.message); process.exitCode = 1;
});
