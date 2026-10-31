use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PageDoc {
    pub filename: String,
    pub title: String,
    pub content: String,
}

pub struct KnowledgeBase {
    pub pages: Vec<PageDoc>,
}

impl KnowledgeBase {
    /// Dynamically loads all .md files from any given directory.
    /// Works with ANY project simply by changing or adding .md files!
    pub fn load_from_dir<P: AsRef<Path>>(dir_path: P) -> Self {
        let mut pages = Vec::new();
        let path = dir_path.as_ref();

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path.is_file() && file_path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Ok(content) = fs::read_to_string(&file_path) {
                        let filename = file_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or_default()
                            .to_string();

                        let title = content
                            .lines()
                            .find(|line| line.starts_with("# "))
                            .map(|line| line.trim_start_matches("# ").trim().to_string())
                            .unwrap_or_else(|| filename.clone());

                        println!("Loaded knowledge page: {} ({})", filename, title);
                        pages.push(PageDoc {
                            filename,
                            title,
                            content,
                        });
                    }
                }
            }
        }

        println!("Loaded {} knowledge markdown pages into memory.", pages.len());
        Self { pages }
    }

    /// Generic, TF-IDF inspired full-text search with dynamic weighting.
    /// Zero hardcoded page names! Matches ANY project's markdown documents automatically.
    pub fn search(&self, query: &str, limit: usize) -> Vec<PageDoc> {
        if self.pages.is_empty() {
            return Vec::new();
        }

        let query_clean: String = query
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
            .collect();

        let words: Vec<String> = query_clean
            .split_whitespace()
            .filter(|w| w.len() > 1)
            .map(|w| w.to_string())
            .collect();

        let mut scored: Vec<(usize, &PageDoc)> = self
            .pages
            .iter()
            .map(|page| {
                let fname_lower = page.filename.to_lowercase();
                let title_lower = page.title.to_lowercase();
                let body_lower = page.content.to_lowercase();

                let mut score: usize = 0;

                // 1. Exact phrase matches in Title or Filename (Highest relevancy)
                if !query_clean.is_empty() {
                    if title_lower.contains(&query_clean) {
                        score += 80;
                    }
                    if fname_lower.contains(&query_clean) {
                        score += 90;
                    }
                }

                // 2. Individual word relevancy scoring
                for w in &words {
                    // Filename exact or prefix match (Strongest signal)
                    if fname_lower == *w {
                        score += 70;
                    } else if fname_lower.contains(w) {
                        score += 35;
                    }

                    // Title word match
                    if title_lower.contains(w) {
                        score += 25;
                    }

                    // Content frequency occurrences (capped to avoid noise)
                    let count = body_lower.matches(w.as_str()).count();
                    score += count.min(15) * 3;
                }

                (score, page)
            })
            .collect();

        // Sort descending by calculated relevance score
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        // If top scored has positive score, return top matches
        let results: Vec<PageDoc> = scored
            .into_iter()
            .filter(|(s, _)| *s > 0)
            .take(limit)
            .map(|(_, p)| p.clone())
            .collect();

        // Fallback: If no specific word matched (e.g. general query like "hi" or "help"),
        // return the first available pages so the AI still has context to assist
        if results.is_empty() {
            self.pages.iter().take(limit).cloned().collect()
        } else {
            results
        }
    }
}
