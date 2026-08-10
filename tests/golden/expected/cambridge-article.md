---
id: cambridge-article
source_format: cambridge-html
title: "Turbulent Flow Simulations Using Lattice Boltzmann Methods"
authors:
  - "Petrov, Alexei"
  - "Huang, Li"
doi: "10.1017/jfm.2024.1087"
journal: "Journal of Fluid Mechanics"
---

# Turbulent Flow Simulations Using Lattice Boltzmann Methods

## Abstract

We present high-resolution **lattice Boltzmann** simulations of turbulent channel flow at Re_τ = 1000.

## Numerical Method

The lattice Boltzmann equation discretizes the *Boltzmann transport equation* on a regular lattice with discrete velocity sets.

### D3Q19 Model

The three-dimensional model uses 19 discrete velocities per node, providing sufficient isotropy for Navier-Stokes recovery.

## Results

Mean velocity profiles match DNS data within 0.5% across the channel width.

| Statistic | LBM | DNS | Error (%) |
| --- | --- | --- | --- |
| uτ | 0.04187 | 0.04180 | 0.17 |
| Reτ | 1002 | 1000 | 0.20 |

