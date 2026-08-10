---
id: oup-journal-deep-headings
source_format: oup-journal-html
title: "Hierarchical Organization of the Mammalian Brain Connectome"
authors:
  - "Fernandez, Diego"
doi: "10.1093/cercor/bhae234"
journal: "Cerebral Cortex"
---

# Hierarchical Organization of the Mammalian Brain Connectome

## Introduction

The brain connectome exhibits **hierarchical modularity** across multiple spatial scales.

## Methods

### Data Acquisition

#### Diffusion Tensor Imaging

DTI data were acquired at 1.25 mm isotropic resolution using a multiband sequence.

#### Functional MRI

Resting-state fMRI was collected over four 15-minute runs per participant.

##### Preprocessing Pipeline

Data were processed using fMRIPrep v23.1 with default parameters.

##### Quality Control

Participants with mean framewise displacement exceeding 0.3 mm were excluded.

### Network Construction

#### Parcellation

We used the Schaefer 400-parcel atlas aligned to the MNI152 template.

#### Edge Weights

Structural edges were weighted by streamline count; functional edges by Fisher-transformed correlations.

## Results

Community detection revealed *four hierarchical levels* of organization, from individual parcels to whole-brain modules.

| Level | Communities | Modularity Q |
| --- | --- | --- |
| 1 (finest) | 42 | 0.71 |
| 2 | 12 | 0.58 |
| 3 | 5 | 0.43 |
| 4 (coarsest) | 2 | 0.31 |

