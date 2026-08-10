# Golden file sources

Real publisher HTML and XML used as test fixtures. Re-fetch periodically
to detect publisher platform changes that break extraction.

## Publisher HTML

| File | Publisher | Source URL | Fetched |
|------|-----------|-----------|---------|
| plos-real-oa-coverage.html | PLOS ONE | https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0320347 | 2026-08-09 |
| plos-real-editorial.html | PLOS ONE | https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0308492 | 2026-08-09 |
| plosbio-real-article.html | PLOS Biology | https://journals.plos.org/plosbiology/article?id=10.1371/journal.pbio.3002700 | 2026-08-09 |
| springer-natcomm-real.html | Nature Communications (Springer) | https://link.springer.com/article/10.1038/s41467-024-45563-x | 2026-08-09 |

## JATS XML (via PubMed Central)

| File | Journal | PMC ID | Source URL | Fetched |
|------|---------|--------|-----------|---------|
| jats-real-neuroscience.xml | Neuroscience | PMC10965040 | https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pmc&id=10965040&rettype=xml | 2026-08-09 |
| jats-real-nihr.xml | NIHR Open Research | PMC7612280 | https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pmc&id=7612280&rettype=xml | 2026-08-09 |
| jats-real-schizophrenia.xml | Schizophrenia | PMC10899210 | https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pmc&id=10899210&rettype=xml | 2026-08-09 |
| jats-real-editorial.xml | Frontiers in Systems Neuroscience | PMC11112070 | https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pmc&id=11112070&rettype=xml | 2026-08-09 |

## Publishers needing browser fetch (Cloudflare-blocked for curl)

These were verified in-browser but could not be saved programmatically.
Re-fetch manually via Save As or with a future crawl script.

| Publisher | Verified URL | Status |
|-----------|-------------|--------|
| BMC (Springer platform) | https://bmcgenomics.biomedcentral.com/articles/10.1186/s12864-024-10069-9 | verified, needs manual save |
| Wiley | https://onlinelibrary.wiley.com/doi/full/10.1002/ece3.70046 | verified, needs manual save |
| OUP | https://academic.oup.com/nar/article/52/D1/D419/7331021 | verified, needs manual save |
| Cambridge | https://www.cambridge.org/core/journals/european-review/article/academic-publishing-in-modern-society/0EC1DDE5C7890B022B06C6B36C257EFD | verified, needs manual save |
| T&F | https://www.tandfonline.com/doi/full/10.1080/08820538.2024.2333644 | verified, needs manual save |
| MDPI | https://www.mdpi.com/1424-8220/24/4/1234 | verified, needs manual save |
| Frontiers | https://www.frontiersin.org/journals/neuroscience/articles | new Nuxt platform, /full redirects to /abstract |
