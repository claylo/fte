use std::path::Path;

use fte::config::Config;
use fte::detect;
use fte::epub;
use fte::extract;

fn golden(input_name: &str) {
    let input_path = Path::new("tests/golden/input").join(input_name);
    let content = std::fs::read_to_string(&input_path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", input_path.display()));

    let id = input_path.file_stem().unwrap().to_str().unwrap();
    let config = Config::default();
    let format = detect::detect_format(&input_path, &content, &config);

    let actual = extract::extract(&format, id, &content)
        .unwrap_or_else(|e| panic!("extracting {input_name}: {e}"));

    check_golden(input_name, id, &actual);
}

fn golden_epub(input_name: &str) {
    let input_path = Path::new("tests/golden/input").join(input_name);
    let id = input_path.file_stem().unwrap().to_str().unwrap();

    let actual = epub::extract(id, &input_path, &Config::default().epub)
        .unwrap_or_else(|e| panic!("extracting {input_name}: {e}"));

    check_golden(input_name, id, &actual);
}

fn check_golden(input_name: &str, id: &str, actual: &str) {
    let expected_name = format!("{}.md", id);
    let expected_path = Path::new("tests/golden/expected").join(&expected_name);

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(&expected_path, actual)
            .unwrap_or_else(|e| panic!("writing {}: {e}", expected_path.display()));
        return;
    }

    let expected = std::fs::read_to_string(&expected_path).unwrap_or_else(|e| {
        panic!(
            "reading {}: {e}\n\nRun with UPDATE_GOLDEN=1 to create it",
            expected_path.display()
        )
    });

    if actual != expected {
        // Show a useful diff
        let mut diff = String::new();
        for change in diff::lines(&expected, actual) {
            match change {
                diff::Result::Left(l) => diff.push_str(&format!("- {l}\n")),
                diff::Result::Right(r) => diff.push_str(&format!("+ {r}\n")),
                diff::Result::Both(b, _) => diff.push_str(&format!("  {b}\n")),
            }
        }
        panic!(
            "golden file mismatch for {input_name}:\n\n{diff}\n\n\
             Run with UPDATE_GOLDEN=1 to accept the new output"
        );
    }
}

fn golden_bail(input_name: &str, expected_fragment: &str) {
    let input_path = Path::new("tests/golden/input").join(input_name);
    let content = std::fs::read_to_string(&input_path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", input_path.display()));

    let id = input_path.file_stem().unwrap().to_str().unwrap();
    let config = Config::default();
    let format = detect::detect_format(&input_path, &content, &config);

    let err =
        extract::extract(&format, id, &content).expect_err(&format!("{input_name} should bail"));

    assert!(
        err.to_string().contains(expected_fragment),
        "{input_name}: expected error containing {expected_fragment:?}, got: {err}"
    );
}

// --- Springer ---

#[test]
fn springer_article() {
    golden("springer-article.html");
}

#[test]
fn springer_chapter() {
    golden("springer-chapter.html");
}

// --- Wiley HTML ---

#[test]
fn wiley_html_article() {
    golden("wiley-html-article.html");
}

#[test]
fn wiley_html_review() {
    golden("wiley-html-review.html");
}

// --- Taylor & Francis ---

#[test]
fn tandf_fulltext() {
    golden("tandf-fulltext.html");
}

#[test]
fn tandf_abstract_only_bails() {
    golden_bail("tandf-abstract-only.html", "abstract-only");
}

// --- OUP Book ---

#[test]
fn oup_book_chapter() {
    golden("oup-book-chapter.html");
}

#[test]
fn oup_book_handbook() {
    golden("oup-book-handbook.html");
}

// --- OUP Journal ---

#[test]
fn oup_journal_article() {
    golden("oup-journal-article.html");
}

#[test]
fn oup_journal_review() {
    golden("oup-journal-review.html");
}

// --- Cambridge ---

#[test]
fn cambridge_article() {
    golden("cambridge-article.html");
}

#[test]
fn cambridge_no_content_bails() {
    golden_bail("cambridge-no-content.html", "no-content");
}

// --- PLOS ---

#[test]
fn plos_research() {
    golden("plos-research.html");
}

#[test]
fn plos_short() {
    golden("plos-short.html");
}

// --- SAGE HTML (abstract-only) ---

#[test]
fn sage_html_bails() {
    golden_bail("sage-html-article.html", "abstract-only");
}

// --- JATS XML ---

#[test]
fn jats_research() {
    golden("jats-research.xml");
}

#[test]
fn jats_sectioned_abstract() {
    golden("jats-sectioned-abstract.xml");
}

// --- Wiley XML ---

#[test]
fn wiley_xml_article() {
    golden("wiley-xml-article.xml");
}

#[test]
fn wiley_xml_review() {
    golden("wiley-xml-review.xml");
}

// --- ACS ---

#[test]
fn acs_article() {
    golden("acs-article.html");
}

#[test]
fn acs_review() {
    golden("acs-review.html");
}

// --- BMC ---

#[test]
fn bmc_article() {
    golden("bmc-article.html");
}

#[test]
fn bmc_short() {
    golden("bmc-short.html");
}

// --- Elsevier ---

#[test]
fn elsevier_article() {
    golden("elsevier-article.html");
}

#[test]
fn elsevier_review() {
    golden("elsevier-review.html");
}

// --- Frontiers ---

#[test]
fn frontiers_article() {
    golden("frontiers-article.html");
}

#[test]
fn frontiers_review() {
    golden("frontiers-review.html");
}

// --- IEEE ---

#[test]
fn ieee_article() {
    golden("ieee-article.html");
}

#[test]
fn ieee_conference() {
    golden("ieee-conference.html");
}

// --- MDPI ---

#[test]
fn mdpi_article() {
    golden("mdpi-article.html");
}

#[test]
fn mdpi_short() {
    golden("mdpi-short.html");
}

// --- Fallback/Other ---

#[test]
fn fallback_generic() {
    golden("fallback-generic.html");
}

#[test]
fn fallback_main() {
    golden("fallback-main.html");
}

// --- Additional synthetic fixtures ---

#[test]
fn springer_cruft_heavy() {
    golden("springer-cruft-heavy.html");
}

#[test]
fn springer_minimal_meta() {
    golden("springer-minimal-meta.html");
}

#[test]
fn springer_unicode_meta() {
    golden("springer-unicode-meta.html");
}

#[test]
fn wiley_html_cruft() {
    golden("wiley-html-cruft.html");
}

#[test]
fn tandf_complex_tables() {
    golden("tandf-complex-tables.html");
}

#[test]
fn oup_journal_deep_headings() {
    golden("oup-journal-deep-headings.html");
}

#[test]
fn cambridge_with_refs() {
    golden("cambridge-with-refs.html");
}

#[test]
fn plos_figures_lists() {
    golden("plos-figures-lists.html");
}

#[test]
fn elsevier_nested_formatting() {
    golden("elsevier-nested-formatting.html");
}

#[test]
fn ieee_many_authors() {
    golden("ieee-many-authors.html");
}

#[test]
fn frontiers_long_body() {
    golden("frontiers-long-body.html");
}

#[test]
fn mdpi_multi_section() {
    golden("mdpi-multi-section.html");
}

#[test]
fn acs_special_chars() {
    golden("acs-special-chars.html");
}

#[test]
fn jats_tables_figures() {
    golden("jats-tables-figures.xml");
}

#[test]
fn jats_no_body() {
    golden("jats-no-body.xml");
}

#[test]
fn jats_element_citations() {
    golden("jats-element-citations.xml");
}

#[test]
fn wiley_xml_cals_tables() {
    golden("wiley-xml-cals-tables.xml");
}

#[test]
fn wiley_xml_flat_abstract() {
    golden("wiley-xml-flat-abstract.xml");
}

// --- Real publisher HTML ---

#[test]
fn plos_real_oa_coverage() {
    golden("plos-real-oa-coverage.html");
}

#[test]
fn plos_real_editorial() {
    golden("plos-real-editorial.html");
}

#[test]
fn plosbio_real_article() {
    golden("plosbio-real-article.html");
}

#[test]
fn springer_natcomm_real() {
    golden("springer-natcomm-real.html");
}

// --- Real JATS XML (from PubMed Central) ---

#[test]
fn jats_real_neuroscience() {
    golden("jats-real-neuroscience.xml");
}

#[test]
fn jats_real_nihr() {
    golden("jats-real-nihr.xml");
}

#[test]
fn jats_real_schizophrenia() {
    golden("jats-real-schizophrenia.xml");
}

#[test]
fn jats_real_editorial() {
    golden("jats-real-editorial.xml");
}

// --- Real publisher HTML (manually saved) ---

#[test]
fn bmcgenomics_real() {
    golden("bmcgenomics-passionfruit.html");
}

#[test]
fn wiley_real_forest_defoliation() {
    golden("wiley-forest-defoliation.html");
}

#[test]
fn oup_real_dbapis() {
    golden("oup-dbapis.html");
}

#[test]
fn cambridge_real_academic_publishing() {
    golden("cambridge-academic-publishing.html");
}

#[test]
fn tandf_real_readership_awareness() {
    golden("taylor-francis-readership-awareness.html");
}

#[test]
fn mdpi_real_polymer_waveguide() {
    golden("mdpi-polymer-waveguide-sensor.html");
}

#[test]
fn frontiers_real_angelshark() {
    golden("frontiers-angelshark-real.html");
}

// --- Real JATS XML (Frontiers via PubMed DTD) ---

#[test]
fn frontiers_real_angelshark_xml() {
    golden("frontiers-angelshark.xml");
}

// --- Real ePub ---

#[test]
fn tandf_real_readership_awareness_epub() {
    golden_epub("tandf-epub-readership-awareness.epub");
}

#[test]
fn frontiers_real_angelshark_epub() {
    golden_epub("frontiers-epub-angelshark.epub");
}
