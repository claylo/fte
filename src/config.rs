use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Top-level configuration.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub input_dir: String,
    pub output_dir: String,
    pub publishers: BTreeMap<String, PublisherProfile>,
    pub fallback: PublisherProfile,
}

/// A publisher extraction profile — detection markers + CSS selectors.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PublisherProfile {
    /// AND logic — all must match in the first 150KB.
    pub detect: Vec<String>,
    /// OR logic — at least one must match in the first 150KB.
    pub detect_any: Vec<String>,
    /// CSS selectors tried in order for the body container.
    pub body_selectors: Vec<String>,
    /// CSS selectors for elements to strip before extraction.
    pub cruft_selectors: Vec<String>,
    /// CSS selector for the references section (stripped from body output).
    pub ref_selector: Option<String>,
    /// If set, bail when this string is absent (e.g. T&F fulltext marker).
    pub fulltext_required: Option<String>,
    /// If set, bail when this string is present AND content_marker is absent.
    pub no_content_marker: Option<String>,
    /// Paired with no_content_marker — its presence overrides the bail.
    pub content_marker: Option<String>,
    /// Always bail (e.g. SAGE HTML is abstract-only).
    pub abstract_only: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input_dir: "ref/epub".into(),
            output_dir: "ref/epub-md".into(),
            publishers: default_publishers(),
            fallback: fallback_profile(),
        }
    }
}

fn fallback_profile() -> PublisherProfile {
    PublisherProfile {
        body_selectors: vec![
            "article".into(),
            "main".into(),
            "div.article-body".into(),
            "div.content".into(),
        ],
        cruft_selectors: vec![
            "nav".into(),
            "footer".into(),
            "header".into(),
            "script".into(),
            "style".into(),
            "div.sidebar".into(),
            "aside".into(),
        ],
        ..Default::default()
    }
}

fn default_publishers() -> BTreeMap<String, PublisherProfile> {
    let mut m = BTreeMap::new();

    m.insert(
        "springer".into(),
        PublisherProfile {
            detect_any: vec!["c-article-body".into(), "c-article-title".into()],
            body_selectors: vec![
                "div.c-article-body".into(),
                "main.c-chapter-body".into(),
                "article#main".into(),
            ],
            cruft_selectors: vec![
                "div.c-article-recommendations".into(),
                "aside.c-article-sidebar".into(),
                "nav".into(),
                "footer".into(),
                "header.eds-c-header".into(),
                "header.c-header".into(),
                r#"div[data-test="cobranding-download"]"#.into(),
                r#"section[data-title="Inline Recommendations"]"#.into(),
                "div#MagazineFulltextArticleBodySuffix".into(),
                "div.c-article-author-institutional-author".into(),
                "script".into(),
                "style".into(),
                "div.c-article-identifiers".into(),
            ],
            ref_selector: Some(r#"section[data-title="References"]"#.into()),
            ..Default::default()
        },
    );

    m.insert(
        "wiley-html".into(),
        PublisherProfile {
            detect_any: vec![
                "article__content".into(),
                "onlinelibrary.wiley.com".into(),
                "onlinelibrary-wiley-com".into(),
            ],
            body_selectors: vec![
                "div#article__content".into(),
                "div.article__body".into(),
                r#"article[lang="en"]"#.into(),
            ],
            cruft_selectors: vec![
                "div.advert".into(),
                "div.main-footer".into(),
                "div.banner-wrapper".into(),
                "nav".into(),
                "div.mange-cookies-btn".into(),
                "div.article-tools__block".into(),
                "div.coolBar".into(),
                "div.articleAdvert".into(),
                "script".into(),
                "style".into(),
            ],
            ref_selector: Some("section.article-section__references".into()),
            ..Default::default()
        },
    );

    m.insert(
        "tandf".into(),
        PublisherProfile {
            detect_any: vec![
                "hlFld-Fulltext".into(),
                "hlFld-Abstract".into(),
                "tandfonline.com".into(),
                "tandfonline-com".into(),
            ],
            fulltext_required: Some("hlFld-Fulltext".into()),
            body_selectors: vec!["div.hlFld-Fulltext".into(), "article.article".into()],
            cruft_selectors: vec![
                "div.articleMetricsContainer".into(),
                "div.articleTools".into(),
                "div.widget".into(),
                "div.pb-dropzone".into(),
                "div.articleMeta".into(),
                "script".into(),
                "style".into(),
                "nav".into(),
            ],
            ref_selector: Some("div#references-Section1".into()),
            ..Default::default()
        },
    );

    m.insert(
        "oup-book".into(),
        PublisherProfile {
            detect: vec!["chapter-title".into()],
            detect_any: vec!["academic.oup.com".into(), "academic-oup-com".into()],
            body_selectors: vec![
                "div.content-inner-wrap".into(),
                "div.widget-items--chapters".into(),
            ],
            cruft_selectors: vec![
                "div.ad-banner".into(),
                "div.global-footer".into(),
                "nav".into(),
                "div.info-widget-wrap".into(),
                "div.widget".into(),
                "script".into(),
                "style".into(),
            ],
            ..Default::default()
        },
    );

    m.insert(
        "oup-journal".into(),
        PublisherProfile {
            detect_any: vec![
                "widget-ArticleFulltext".into(),
                "academic.oup.com".into(),
                "academic-oup-com".into(),
            ],
            body_selectors: vec![
                "div.widget-ArticleFulltext".into(),
                "div.article-body".into(),
            ],
            cruft_selectors: vec![
                "div.ad-banner".into(),
                "div.global-footer".into(),
                "nav".into(),
                "div.article-info-wrap".into(),
                "div.section-jump-link".into(),
                "script".into(),
                "style".into(),
            ],
            ref_selector: Some("div.ref-list".into()),
            ..Default::default()
        },
    );

    m.insert(
        "cambridge".into(),
        PublisherProfile {
            detect_any: vec!["cambridge.org".into(), "cambridge-org".into()],
            no_content_marker: Some("no-content".into()),
            content_marker: Some("scrollspy-content".into()),
            body_selectors: vec![
                "div.content-container".into(),
                "div.scrollspy-content".into(),
                "div.article-wrapper".into(),
            ],
            cruft_selectors: vec![
                "div.cited-by-wrapper".into(),
                "div.modal".into(),
                "div.action-bar".into(),
                "div.table-of-content-mobile".into(),
                "script".into(),
                "style".into(),
                "nav".into(),
            ],
            ref_selector: Some("div#references-list".into()),
            ..Default::default()
        },
    );

    m.insert(
        "plos".into(),
        PublisherProfile {
            detect_any: vec!["plos.org".into(), "plos-org".into(), "artText".into()],
            body_selectors: vec!["div#artText".into(), "div.article-text".into()],
            cruft_selectors: vec![
                "ul.article-tabs".into(),
                "div.fig-btns-container".into(),
                "script".into(),
                "style".into(),
                "nav".into(),
            ],
            ..Default::default()
        },
    );

    m.insert(
        "sage-html".into(),
        PublisherProfile {
            detect_any: vec!["sagepub.com".into(), "sagepub-com".into()],
            abstract_only: true,
            ..Default::default()
        },
    );

    m.insert(
        "acs".into(),
        PublisherProfile {
            detect_any: vec![
                "pubs.acs.org".into(),
                "pubs-acs-org".into(),
                "NLM_sec_level".into(),
            ],
            body_selectors: vec![
                "div.article_content-left".into(),
                "div.article_content".into(),
                "div.NLM_sec_level_1".into(),
            ],
            cruft_selectors: vec![
                "div.article_header".into(),
                "div.article_citation".into(),
                "div.articleMetrics".into(),
                "div.support-info".into(),
                "div.article-tools".into(),
                "script".into(),
                "style".into(),
                "nav".into(),
            ],
            ref_selector: Some("div.article_references".into()),
            ..Default::default()
        },
    );

    m.insert(
        "elsevier".into(),
        PublisherProfile {
            detect_any: vec![
                "sciencedirect.com".into(),
                "sciencedirect-com".into(),
                "sd-article".into(),
            ],
            body_selectors: vec![
                "div#body".into(),
                "div.Body".into(),
                "section.Body".into(),
                "div.article-body".into(),
            ],
            cruft_selectors: vec![
                "div.sidebar".into(),
                "div.RelatedContent".into(),
                "div.Appendices".into(),
                "div.article-tools".into(),
                "div.author-info".into(),
                "div.publication-history".into(),
                "script".into(),
                "style".into(),
                "nav".into(),
            ],
            ref_selector: Some("div.References".into()),
            ..Default::default()
        },
    );

    m.insert(
        "frontiers".into(),
        PublisherProfile {
            detect_any: vec!["frontiersin.org".into(), "frontiersin-org".into()],
            body_selectors: vec![
                "div.ArticleContent".into(),
                "div.JournalFullText".into(),
                "div.article-section".into(),
                "article.article-text".into(),
            ],
            cruft_selectors: vec![
                "div.References".into(),
                "div.AbstractSummary".into(),
                "div.article-header".into(),
                "div.article-footer".into(),
                "div.notes-section".into(),
                "button.ArticleReference".into(),
                "script".into(),
                "style".into(),
                "nav".into(),
            ],
            ..Default::default()
        },
    );

    m.insert(
        "ieee".into(),
        PublisherProfile {
            detect_any: vec![
                "ieeexplore.ieee.org".into(),
                "ieeexplore-ieee-org".into(),
            ],
            body_selectors: vec![
                "div#article-content".into(),
                "div.article-content".into(),
                "section.article-content".into(),
            ],
            cruft_selectors: vec![
                "div.stats-document-abstract-publishedIn".into(),
                "div.document-header".into(),
                "div.document-footer".into(),
                "div.article-action-toolbar".into(),
                "div.metrics-section".into(),
                "script".into(),
                "style".into(),
                "nav".into(),
            ],
            ref_selector: Some("div.reference-container".into()),
            ..Default::default()
        },
    );

    m.insert(
        "mdpi".into(),
        PublisherProfile {
            detect_any: vec!["mdpi.com".into(), "mdpi-com".into()],
            body_selectors: vec![
                "div.html-body".into(),
                "div#html-article-content".into(),
                "article.article".into(),
            ],
            cruft_selectors: vec![
                "div.article-icons".into(),
                "div.art-supplementary".into(),
                "div.article-metrics".into(),
                "section.SupplementaryMaterial".into(),
                "script".into(),
                "style".into(),
                "nav".into(),
            ],
            ref_selector: Some("section.html-references".into()),
            ..Default::default()
        },
    );

    m
}
