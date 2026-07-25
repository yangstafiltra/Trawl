use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum SearchEngine {
    #[default]
    Bing,
    Google,
    DuckDuckGo,
}

impl SearchEngine {
    pub fn search_url(&self, query: &str) -> String {
        self.search_url_with_page(query, 1)
    }

    pub fn search_url_with_page(&self, query: &str, page: usize) -> String {
        let encoded: String = query
            .chars()
            .map(|c| match c {
                ' ' => '+',
                c if c.is_alphanumeric() || c == '-' || c == '_' => c,
                c => c,
            })
            .collect();
        match self {
            SearchEngine::Bing => format!("https://www.bing.com/search?q={encoded}&first={}", (page - 1) * 10 + 1),
            SearchEngine::Google => format!("https://www.google.com/search?q={encoded}&start={}", (page - 1) * 10),
            SearchEngine::DuckDuckGo => format!("https://lite.duckduckgo.com/lite?q={encoded}&s={}", (page - 1) * 10),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            SearchEngine::Bing => "Bing",
            SearchEngine::Google => "Google",
            SearchEngine::DuckDuckGo => "DuckDuckGo",
        }
    }

    pub fn is_search_host(host: &str) -> bool {
        host.contains("bing.com") || host.contains("google.") || host.contains("duckduckgo.com")
    }

    pub fn supports_pagination(&self) -> bool {
        false
    }

    pub fn pagination_limit_note(&self) -> Option<&'static str> {
        match self {
            SearchEngine::Bing => Some("Bing 仅第1页（JS渲染限制），按 s 切换引擎"),
            SearchEngine::Google => Some("Google 返回空白（JS渲染限制），按 s 切换引擎"),
            SearchEngine::DuckDuckGo => Some("DuckDuckGo 仅第1页（JS渲染限制），按 s 切换引擎"),
        }
    }

    pub fn next(&self) -> Self {
        match self {
            SearchEngine::Bing => SearchEngine::Google,
            SearchEngine::Google => SearchEngine::DuckDuckGo,
            SearchEngine::DuckDuckGo => SearchEngine::Bing,
        }
    }
}

impl fmt::Display for SearchEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub enum LayoutStyle {
    #[default]
    BrowserChrome,
    LazyGit,
}

impl LayoutStyle {
    pub fn toggle(&self) -> Self {
        match self {
            LayoutStyle::BrowserChrome => LayoutStyle::LazyGit,
            LayoutStyle::LazyGit => LayoutStyle::BrowserChrome,
        }
    }
}

impl fmt::Display for LayoutStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutStyle::BrowserChrome => write!(f, "Browser-Chrome"),
            LayoutStyle::LazyGit => write!(f, "lazygit-Panels"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub home_url: String,
    pub search_engine: SearchEngine,
    pub layout: LayoutStyle,
    pub bookmarks: Vec<Bookmark>,
    pub max_tabs: usize,
    pub max_history: usize,
    pub sidebar_width: u16,
    pub show_images: bool,
    pub download_dir: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            home_url: "trawl:home".into(),
            search_engine: SearchEngine::Bing,
            layout: LayoutStyle::BrowserChrome,
            bookmarks: vec![
                Bookmark {
                    url: "https://www.bing.com".into(),
                    title: "Bing".into(),
                },
                Bookmark {
                    url: "https://github.com".into(),
                    title: "GitHub".into(),
                },
                Bookmark {
                    url: "https://news.ycombinator.com".into(),
                    title: "Hacker News".into(),
                },
                Bookmark {
                    url: "https://en.wikipedia.org".into(),
                    title: "Wikipedia".into(),
                },
                Bookmark {
                    url: "https://lite.duckduckgo.com/lite".into(),
                    title: "DuckDuckGo".into(),
                },
            ],
            max_tabs: 20,
            max_history: 500,
            sidebar_width: 30,
            show_images: true,
            download_dir: None,
        }
    }
}

impl Config {
    pub fn config_dir() -> Option<PathBuf> {
        ProjectDirs::from("com", "trawl", "trawl").map(|d| d.config_dir().to_path_buf())
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir()
            .map(|d| d.join("config.json"))
            .unwrap_or_else(|| PathBuf::from("trawl_config.json"))
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(data) = fs::read_to_string(&path)
            && let Ok(config) = serde_json::from_str(&data)
        {
            return config;
        }
        let config = Config::default();
        let _ = Config::save(&config);
        config
    }

    pub fn save(config: &Config) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let data = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
        fs::write(&path, data).map_err(|e| e.to_string())?;
        Ok(())
    }
}
