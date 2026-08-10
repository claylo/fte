---
id: jats-research
source_format: jats-xml
title: "Graph Neural Networks for Protein Structure Prediction"
authors:
  - "Amir Hosseini"
  - "Sarah O'Brien"
doi: "10.1089/cmb.2024.0123"
journal: "Journal of Computational Biology"
---

# Graph Neural Networks for Protein Structure Prediction

## Abstract

We develop a graph neural network architecture that predicts protein backbone coordinates from sequence with **sub-angstrom accuracy**.

## Introduction

Protein structure prediction has been transformed by deep learning approaches. AlphaFold2 (Jumper et al., 2021) achieved experimental-level accuracy.

## Methods

### Network Architecture

Our model uses an *equivariant* graph neural network with SE(3) symmetry.

### Training Data

We trained on 150,000 structures from the PDB, filtered for resolution better than 2.0 Å.

## Results

Performance metrics on the CASP15 test set:

**Table 1** Prediction accuracy on CASP15 targets

| Metric | Our Model | AlphaFold2 |
| --- | --- | --- |
| GDT-TS | 88.3 | 87.1 |
| TM-score | 0.924 | 0.918 |
| RMSD (Å) | 1.23 | 1.31 |

**Figure 1** Comparison of predicted and experimental structures for target T1024

## References

1. Jumper J, Evans R, Pritzel A, et al. Highly accurate protein structure prediction with AlphaFold. Nature. 2021;596:583-589.
2. Baek M, DiMaio F, Anishchenko I, et al. Accurate prediction of protein structures and interactions using a three-track neural network. Science. 2021;373:871-876.

