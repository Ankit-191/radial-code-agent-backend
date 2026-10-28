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

    pub fn search(&self, query: &str, limit: usize) -> Vec<PageDoc> {
        if self.pages.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let words: Vec<String> = query_lower
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 2)
            .collect();

        let mut scored: Vec<(usize, &PageDoc)> = self
            .pages
            .iter()
            .map(|page| {
                let fname = page.filename.to_lowercase();
                let title_lower = page.title.to_lowercase();
                let hay = format!("{} {}", page.title, page.content).to_lowercase();
                let mut score = 0;

                // Priority keywords mapping
                if (query_lower.contains("team")
                    || query_lower.contains("member")
                    || query_lower.contains("leader")
                    || query_lower.contains("who work")
                    || query_lower.contains("name"))
                    && fname == "team"
                {
                    score += 150;
                }

                if (query_lower.contains("course")
                    || query_lower.contains("learn")
                    || query_lower.contains("training")
                    || query_lower.contains("class")
                    || query_lower.contains("study")
                    || query_lower.contains("provide"))
                    && (fname == "courses" || fname == "services")
                {
                    if fname == "courses" {
                        score += 150;
                    } else {
                        score += 40;
                    }
                }

                if (query_lower.contains("contact")
                    || query_lower.contains("phone")
                    || query_lower.contains("call")
                    || query_lower.contains("number")
                    || query_lower.contains("email")
                    || query_lower.contains("location")
                    || query_lower.contains("office")
                    || query_lower.contains("address"))
                    && (fname == "contact" || fname == "home")
                {
                    if fname == "contact" {
                        score += 150;
                    } else {
                        score += 50;
                    }
                }

                if (query_lower.contains("service")
                    || query_lower.contains("offer")
                    || query_lower.contains("what do you do")
                    || query_lower.contains("develop"))
                    && fname == "services"
                {
                    score += 150;
                }

                if (query_lower.contains("career")
                    || query_lower.contains("job")
                    || query_lower.contains("hiring")
                    || query_lower.contains("apply"))
                    && fname == "careers"
                {
                    score += 150;
                }

                if (query_lower.contains("portfolio")
                    || query_lower.contains("project")
                    || query_lower.contains("case study")
                    || query_lower.contains("work"))
                    && fname == "portfolio"
                {
                    score += 150;
                }

                for w in &words {
                    if fname.contains(w) {
                        score += 30;
                    }
                    if title_lower.contains(w) {
                        score += 20;
                    }
                    let count = hay.matches(w.as_str()).count();
                    score += count.min(15);
                }

                (score, page)
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));

        scored
            .into_iter()
            .take(limit)
            .map(|(_, p)| p.clone())
            .collect()
    }
}
