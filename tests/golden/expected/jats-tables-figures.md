---
id: jats-tables-figures
source_format: jats-xml
title: "Single-Cell Transcriptomics Reveals Tumor Microenvironment Heterogeneity"
authors:
  - "Mei Zhang"
  - "Rajesh Kumar"
doi: "10.1038/s41467-024-50123-4"
journal: "Nature Communications"
---

# Single-Cell Transcriptomics Reveals Tumor Microenvironment Heterogeneity

## Abstract

We performed single-cell RNA sequencing on 120,000 cells from 18 *treatment-naïve* colorectal tumors, identifying **23 distinct cell states** within the tumor microenvironment.

## Introduction

Tumor heterogeneity drives treatment resistance and disease progression. Single-cell technologies enable characterization of individual cell states within the tumor ecosystem (Zhang et al., 2020).

## Results

### Cell Type Composition

Unsupervised clustering identified major cell populations:

**Table 1** Cell type proportions across tumor samples

| Cell Type | Mean % | Range |
| --- | --- | --- |
| Epithelial (tumor) | 42.3 | 28–61 |
| T cells | 18.7 | 8–32 |
| Myeloid | 14.2 | 6–24 |
| Fibroblasts | 12.1 | 5–22 |
| B cells | 7.8 | 2–15 |
| Endothelial | 4.9 | 1–11 |

**Figure 1** UMAP visualization of 120,000 cells colored by cell type annotation

### T Cell States

Within the T cell compartment, we identified *exhausted* CD8^+ T cells (PD-1^hi, TIM-3^hi, LAG-3^+) enriched in **microsatellite-stable** tumors.

**Table 2** Exhaustion marker expression by MSI status

| Marker | MSI-H | MSS | p-value |
| --- | --- | --- | --- |
| PD-1 | 28% | 52% | <0.001 |
| TIM-3 | 15% | 41% | <0.001 |
| LAG-3 | 8% | 23% | 0.003 |

**Figure 2** Pseudotime trajectory analysis of CD8^+ T cell differentiation from naïve to exhausted states

## Discussion

Our data reveal that the tumor microenvironment in colorectal cancer is **more heterogeneous** than previously appreciated, with implications for immunotherapy patient stratification.

## References

1. Zhang L, Li Z, Skrber K, et al. Single-cell analyses inform mechanisms of myeloid-targeted therapies in colon cancer. Cell. 2020;181:442-459.
2. PelkaK HofreeM Spatially organized multicellular immune hubs in human colorectal cancer Cell 2021 184 4734 4752
3. Tirosh I, Izar B, Prakadan SM, et al. Dissecting the multicellular ecosystem of metastatic melanoma by single-cell RNA-seq. Science. 2016;352:189-196.

