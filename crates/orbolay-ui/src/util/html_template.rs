use std::{
  fmt::Write,
  fs,
  path::{Path, PathBuf},
  sync::{Mutex, OnceLock},
  time::SystemTime,
};

use log::warn;
use orbolay_core::{config::config_dir, dirs::TEMPLATES_DIR};
use scraper::{Html, Selector};

use crate::util::theme::{Theme, to_hex};

/// Rules shared by one family of templates (voice users or notifications).
pub struct TemplateSpec {
  pub kind: &'static str,
  pub dir: &'static str,
  pub root_id: &'static str,
  pub image_token: &'static str,
  pub required_tokens: &'static [&'static str],
  pub builtin: &'static str,
}

/// A validated template whose root element can be rewritten per render.
#[derive(Clone)]
pub struct Template {
  html: String,
  root_name: String,
  root_attrs: Vec<(String, String)>,
}

impl Template {
  /// Returns the document with state classes and attributes added to the root element and the
  /// theme variables injected into `<head>`.
  pub fn render_root(
    &self,
    classes: &[&str],
    extra_attrs: Vec<(String, String)>,
    theme: &Theme,
  ) -> String {
    let root_tag = render_tag(&self.root_name, &self.root_attrs);
    let mut attrs = merge_classes(&self.root_attrs, classes);
    attrs.extend(extra_attrs);
    let new_root_tag = render_tag(&self.root_name, &attrs);

    self.html.replace(&root_tag, &new_root_tag).replace(
      "</head>",
      &format!("<style>:root{{{}}}</style></head>", theme_decls(theme)),
    )
  }
}

struct Cached {
  name: String,
  mtime: Option<SystemTime>,
  template: Template,
}

/// Loads named templates from the config directory, falling back to the built-in one.
pub struct TemplateStore {
  spec: TemplateSpec,
  builtin: OnceLock<Template>,
  cache: Mutex<Option<Cached>>,
}

impl TemplateStore {
  pub const fn new(spec: TemplateSpec) -> Self {
    Self {
      spec,
      builtin: OnceLock::new(),
      cache: Mutex::new(None),
    }
  }

  fn builtin(&self) -> Template {
    self
      .builtin
      .get_or_init(|| {
        preprocess(&self.spec, self.spec.builtin).expect("the built-in template must be valid")
      })
      .clone()
  }

  fn dir(&self) -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(TEMPLATES_DIR).join(self.spec.dir))
  }

  pub fn list(&self) -> Vec<String> {
    let Some(entries) = self.dir().and_then(|dir| fs::read_dir(dir).ok()) else {
      return Vec::new();
    };
    let mut names = entries
      .flatten()
      .map(|entry| entry.path())
      .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "html"))
      .filter_map(|path| path.file_stem()?.to_str().map(str::to_string))
      .collect::<Vec<_>>();
    names.sort();
    names
  }

  pub fn load(&self, name: Option<&str>) -> Template {
    let Some((name, path)) =
      name.and_then(|name| Some((name, self.dir()?.join(format!("{name}.html")))))
    else {
      return self.builtin();
    };

    let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
    let mut cache = self.cache.lock().unwrap();
    if let Some(cached) = cache.as_ref()
      && cached.name == name
      && cached.mtime == mtime
    {
      return cached.template.clone();
    }

    let template = self.read(&path);
    *cache = Some(Cached {
      name: name.to_string(),
      mtime,
      template: template.clone(),
    });
    template
  }

  fn read(&self, path: &Path) -> Template {
    let kind = self.spec.kind;
    match fs::read_to_string(path).map(|raw| preprocess(&self.spec, &raw)) {
      Ok(Ok(template)) => template,
      Ok(Err(reason)) => {
        warn!(
          "Invalid {kind} template {}: {reason}; using the built-in default",
          path.display()
        );
        self.builtin()
      }
      Err(err) => {
        warn!(
          "Failed to read {kind} template {}: {err}; using the built-in default",
          path.display()
        );
        self.builtin()
      }
    }
  }
}

fn selector(css: &str) -> Selector {
  Selector::parse(css).expect("valid selector")
}

pub fn preprocess(spec: &TemplateSpec, raw: &str) -> Result<Template, String> {
  let doc = Html::parse_document(raw);
  let root_id = spec.root_id;

  let roots: Vec<_> = doc.select(&selector(&format!("#{root_id}"))).collect();
  let [root] = roots.as_slice() else {
    return Err(format!(
      "must contain exactly one element with id=\"{root_id}\" (found {})",
      roots.len()
    ));
  };

  if doc.select(&selector("script")).next().is_some() {
    return Err("JavaScript is not supported: remove the <script> element".into());
  }

  for link in doc.select(&selector("link[href]")) {
    let href = link.attr("href").unwrap_or_default();
    if !is_network_url(href) {
      return Err(format!(
        "stylesheet <link> href must be a network URL (http/https), got: {href}"
      ));
    }
  }

  for img in doc.select(&selector("img[src]")) {
    let src = img.attr("src").unwrap_or_default();
    if src != spec.image_token && !is_network_url(src) {
      return Err(format!(
        "<img> src must be a network URL or {}, got: {src}",
        spec.image_token
      ));
    }
  }

  if let Some(token) = spec.required_tokens.iter().find(|t| !raw.contains(**t)) {
    return Err(format!("must contain the {token} token"));
  }

  let element = root.value();
  Ok(Template {
    html: doc.html(),
    root_name: element.name().to_string(),
    root_attrs: element
      .attrs()
      .map(|(name, value)| (name.to_string(), value.to_string()))
      .collect(),
  })
}

pub fn escape_html(input: &str) -> String {
  input
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#39;")
}

pub fn escape_attr(value: &str) -> String {
  let mut out = String::with_capacity(value.len());
  for c in value.chars() {
    match c {
      '&' => out.push_str("&amp;"),
      '\u{00A0}' => out.push_str("&nbsp;"),
      '"' => out.push_str("&quot;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      c => out.push(c),
    }
  }
  out
}

pub fn render_tag(name: &str, attrs: &[(String, String)]) -> String {
  let mut tag = String::with_capacity(64);
  tag.push('<');
  tag.push_str(name);
  for (name, value) in attrs {
    let _ = write!(tag, " {name}=\"{}\"", escape_attr(value));
  }
  tag.push('>');
  tag
}

pub fn merge_classes(attrs: &[(String, String)], extra: &[&str]) -> Vec<(String, String)> {
  let class_pos = attrs.iter().position(|(name, _)| name == "class");
  let mut classes: Vec<&str> =
    class_pos.map_or_else(Vec::new, |pos| attrs[pos].1.split_whitespace().collect());
  for class in extra {
    if !classes.contains(class) {
      classes.push(class);
    }
  }

  if classes.is_empty() && class_pos.is_none() {
    return attrs.to_vec();
  }

  let class_attr = ("class".to_string(), classes.join(" "));
  let mut out = attrs.to_vec();
  match class_pos {
    Some(pos) => out[pos] = class_attr,
    None => out.push(class_attr),
  }
  out
}

pub fn is_network_url(url: &str) -> bool {
  url.starts_with("http://") || url.starts_with("https://")
}

pub fn theme_decls(theme: &Theme) -> String {
  format!(
    "--gray: {}; --darkish-gray: {}; --light-gray: {}; --superlight-gray: {}; --muted-gray: {}; --text: {}; --green: #01863B; --border-radius: {}px;",
    to_hex(theme.gray),
    to_hex(theme.darkish_gray),
    to_hex(theme.light_gray),
    to_hex(theme.superlight_gray),
    to_hex(theme.muted_gray),
    to_hex(theme.text_color),
    theme.border_radius
  )
}

#[cfg(test)]
mod tests {
  use super::{TemplateSpec, merge_classes, preprocess};

  const SPEC: TemplateSpec = TemplateSpec {
    kind: "test",
    dir: "test",
    root_id: "root",
    image_token: "{{img}}",
    required_tokens: &["{{name}}"],
    builtin: "",
  };

  fn attrs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
      .iter()
      .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
      .collect()
  }

  fn rejection(html: &str) -> String {
    preprocess(&SPEC, html)
      .err()
      .expect("template should be rejected")
  }

  #[test]
  fn accepts_a_minimal_template() {
    assert!(preprocess(&SPEC, r#"<div id="root">{{name}}<img src="{{img}}"></div>"#).is_ok());
  }

  #[test]
  fn requires_exactly_one_root() {
    assert!(rejection("<div>{{name}}</div>").contains("found 0"));
    assert!(rejection(r#"<p id="root"></p><p id="root">{{name}}</p>"#).contains("found 2"));
  }

  #[test]
  fn rejects_scripts_local_resources_and_missing_tokens() {
    assert!(rejection(r#"<div id="root">{{name}}<script></script></div>"#).contains("JavaScript"));
    assert!(
      rejection(r#"<link href="file:///x.css"><div id="root">{{name}}</div>"#).contains("<link>")
    );
    assert!(rejection(r#"<div id="root">{{name}}<img src="/x.png"></div>"#).contains("<img>"));
    assert!(rejection(r#"<div id="root"></div>"#).contains("{{name}}"));
  }

  #[test]
  fn merge_classes_keeps_attribute_order_and_deduplicates() {
    assert_eq!(
      merge_classes(&attrs(&[("id", "root"), ("class", "a b")]), &["b", "c"]),
      attrs(&[("id", "root"), ("class", "a b c")])
    );
    assert_eq!(
      merge_classes(&attrs(&[("id", "root")]), &["c"]),
      attrs(&[("id", "root"), ("class", "c")])
    );
    assert_eq!(
      merge_classes(&attrs(&[("id", "root")]), &[]),
      attrs(&[("id", "root")])
    );
  }
}
