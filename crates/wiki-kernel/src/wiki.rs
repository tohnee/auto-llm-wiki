use chrono::Utc;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::engine::Result;

pub fn ensure_layout(wiki_dir: &Path) -> Result<()> {
    fs::create_dir_all(wiki_dir.join("pages"))?;
    fs::create_dir_all(wiki_dir.join("concepts"))?;
    fs::create_dir_all(wiki_dir.join("reports"))?;

    ensure_file(
        &wiki_dir.join("index.md"),
        "# Wiki Index\n\nGenerated pages appear here.\n",
    )?;
    ensure_file(&wiki_dir.join("log.md"), "# Wiki Log\n\n")?;
    Ok(())
}

pub fn write_page(wiki_dir: &Path, slug: &str, title: &str, body: &str) -> Result<PathBuf> {
    ensure_layout(wiki_dir)?;
    let path = wiki_dir.join("pages").join(format!("{slug}.md"));
    fs::write(&path, body)?;
    append_once(&wiki_dir.join("index.md"), &format!("- [[{slug}]]\n"))?;
    append_once(
        &wiki_dir.join("log.md"),
        &format!("{} page_written {title}\n", Utc::now().to_rfc3339()),
    )?;
    Ok(path)
}

pub fn write_report(wiki_dir: &Path, slug: &str, body: &str) -> Result<PathBuf> {
    ensure_layout(wiki_dir)?;
    let path = wiki_dir.join("reports").join(format!("{slug}.md"));
    fs::write(&path, body)?;
    append_once(
        &wiki_dir.join("log.md"),
        &format!("{} report_written {slug}\n", Utc::now().to_rfc3339()),
    )?;
    Ok(path)
}

pub fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "untitled".to_owned()
    } else {
        slug.to_owned()
    }
}

fn ensure_file(path: &Path, default_body: &str) -> Result<()> {
    if !path.exists() {
        fs::write(path, default_body)?;
    }
    Ok(())
}

fn append_once(path: &Path, line: &str) -> Result<()> {
    let mut content = fs::read_to_string(path)?;
    if !content.contains(line) {
        content.push_str(line);
        fs::write(path, content)?;
    }
    Ok(())
}
