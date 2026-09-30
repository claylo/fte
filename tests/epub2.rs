//! Book-length extraction from EPUB2 files, which carry their table of
//! contents in an NCX (`toc.ncx`) instead of an EPUB3 nav document.
//!
//! The fixture is built in a temp dir rather than checked in: publisher
//! EPUB2 books are copyrighted, and the structure is what matters here —
//! `version="2.0"`, `<spine toc="ncx">`, `.html` spine files, and a nested
//! part → chapter → section NCX whose section entries point at fragments.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use zip::write::SimpleFileOptions;

const CONTAINER: &str = r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

const OPF: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<package version="2.0" unique-identifier="bookid" xmlns="http://www.idpf.org/2007/opf">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
<dc:title>Field Notes on Survey Practice</dc:title>
<dc:creator opf:role="aut">A. Surveyor</dc:creator>
<dc:identifier id="bookid" opf:scheme="ISBN">9780000000000</dc:identifier>
</metadata>
<manifest>
<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
<item id="cover" href="cover.html" media-type="application/xhtml+xml"/>
<item id="title" href="title.html" media-type="application/xhtml+xml"/>
<item id="content" href="contents.html" media-type="application/xhtml+xml"/>
<item id="part1" href="part1.html" media-type="application/xhtml+xml"/>
<item id="chapter1" href="chapter1.html" media-type="application/xhtml+xml"/>
<item id="chapter2" href="chapter2.html" media-type="application/xhtml+xml"/>
<item id="part2" href="part2.html" media-type="application/xhtml+xml"/>
<item id="chapter3" href="chapter3.html" media-type="application/xhtml+xml"/>
</manifest>
<spine toc="ncx">
<itemref idref="cover"/>
<itemref idref="title"/>
<itemref idref="content"/>
<itemref idref="part1"/>
<itemref idref="chapter1"/>
<itemref idref="chapter2"/>
<itemref idref="part2"/>
<itemref idref="chapter3"/>
</spine>
</package>"#;

const NCX: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd">
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
<head><meta content="2" name="dtb:depth"/></head>
<docTitle><text>Field Notes on Survey Practice</text></docTitle>
<navMap>
<navPoint id="cover" playOrder="1"><navLabel><text>Cover</text></navLabel><content src="cover.html"/></navPoint>
<navPoint id="title" playOrder="2"><navLabel><text>Title Page</text></navLabel><content src="title.html"/></navPoint>
<navPoint id="content" playOrder="3"><navLabel><text>Contents</text></navLabel><content src="contents.html"/></navPoint>
<navPoint id="part1" playOrder="4"><navLabel><text>PART I. Groundwork</text></navLabel><content src="part1.html"/>
<navPoint id="chapter1" playOrder="5"><navLabel><text>Chapter 1. Setting Out</text></navLabel><content src="chapter1.html"/>
<navPoint id="s1" playOrder="6"><navLabel><text>Benchmarks</text></navLabel><content src="chapter1.html#s1"/></navPoint>
<navPoint id="s2" playOrder="7"><navLabel><text>Summary</text></navLabel><content src="chapter1.html#s2"/></navPoint></navPoint>
<navPoint id="chapter2" playOrder="8"><navLabel><text>Chapter 2. Closing the Traverse</text></navLabel><content src="chapter2.html"/></navPoint></navPoint>
<navPoint id="part2" playOrder="9"><navLabel><text>PART II. Practice</text></navLabel><content src="part2.html"/>
<navPoint id="chapter3" playOrder="10"><navLabel><text>Chapter 3. Boundary Evidence</text></navLabel><content src="chapter3.html"/></navPoint></navPoint>
</navMap>
</ncx>"#;

fn page(title: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>{title}</title></head>
<body>
{body}
</body>
</html>"#
    )
}

fn chapter(n: u32, title_html: &str, text: &str) -> String {
    page(
        &format!("chapter{n}"),
        &format!(
            r#"<h1 class="chnum">{n}</h1>
<h1 class="chaptitle">{title_html}</h1>
<p>{text}</p>
<h2 id="s1">Benchmarks</h2>
<p>Every level run starts and ends on a benchmark whose elevation is already known.</p>"#
        ),
    )
}

/// Write the fixture EPUB2 book into `dir` and return its path.
fn build(dir: &Path) -> PathBuf {
    let path = dir.join("field-notes.epub");
    let mut w = zip::ZipWriter::new(File::create(&path).unwrap());
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default();

    let mut add = |name: &str, data: &str, opts: SimpleFileOptions| {
        w.start_file(name, opts).unwrap();
        w.write_all(data.as_bytes()).unwrap();
    };

    add("mimetype", "application/epub+zip", stored);
    add("META-INF/container.xml", CONTAINER, deflated);
    add("OEBPS/content.opf", OPF, deflated);
    add("OEBPS/toc.ncx", NCX, deflated);
    add(
        "OEBPS/cover.html",
        &page("cover", r#"<p>Cover image</p>"#),
        deflated,
    );
    add(
        "OEBPS/title.html",
        &page(
            "title",
            "<h1>Field Notes on Survey Practice</h1><p>A. Surveyor</p>",
        ),
        deflated,
    );
    add(
        "OEBPS/contents.html",
        &page("contents", "<h1>Contents</h1><p>Part I</p><p>Part II</p>"),
        deflated,
    );
    add(
        "OEBPS/part1.html",
        &page(
            "part1",
            r#"<h1 class="partnum">PART I</h1><h1 class="parttitle"><b>Groundwork</b></h1>"#,
        ),
        deflated,
    );
    add(
        "OEBPS/chapter1.html",
        &chapter(
            1,
            "Setting<br/>Out",
            "A traverse begins at a station whose position is fixed before the crew arrives.",
        ),
        deflated,
    );
    add(
        "OEBPS/chapter2.html",
        &chapter(
            2,
            "Closing<br/>the Traverse",
            "A closed traverse returns to its starting station, so its misclosure can be measured.",
        ),
        deflated,
    );
    add(
        "OEBPS/part2.html",
        &page(
            "part2",
            r#"<h1 class="partnum">PART II</h1><h1 class="parttitle"><b>Practice</b></h1>"#,
        ),
        deflated,
    );
    add(
        "OEBPS/chapter3.html",
        &chapter(
            3,
            "Boundary<br/>Evidence",
            "Original monuments control over later measurements when the two disagree.",
        ),
        deflated,
    );

    w.finish().unwrap();
    path
}

#[test]
fn an_epub2_book_takes_its_chapters_from_the_ncx() {
    let tmp = tempfile::tempdir().unwrap();
    let epub = build(tmp.path());

    let md = fte::epub::extract("field-notes", &epub).expect("extracts");
    let doc = fte::chapter::parse(&md);

    let got: Vec<(&str, &str)> = doc
        .chapters
        .iter()
        .map(|c| (c.title.as_str(), c.src.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("PART I. Groundwork", "OEBPS/part1.html"),
            ("Chapter 1. Setting Out", "OEBPS/chapter1.html"),
            ("Chapter 2. Closing the Traverse", "OEBPS/chapter2.html"),
            ("PART II. Practice", "OEBPS/part2.html"),
            ("Chapter 3. Boundary Evidence", "OEBPS/chapter3.html"),
        ]
    );
}

#[test]
fn splitting_an_epub2_book_writes_one_file_per_chapter() {
    let tmp = tempfile::tempdir().unwrap();
    let epub = build(tmp.path());
    let out = tmp.path().join("out");

    Command::cargo_bin("fte")
        .expect("fte binary builds")
        .args(["split", "--subdir", "", "--no-front", "--outdir"])
        .arg(&out)
        .arg(&epub)
        .assert()
        .success();

    let written = std::fs::read_dir(&out).unwrap().count();
    assert_eq!(written, 5);
}
