---
type: spec
title: The Four AI Learning Methods — Visualize, Filter, 80/20, Find Resources
description: Aleks Gornik's 4 practical methods for using AI to learn engineering: make concepts visual with 3D models, filter resources with AI prompts, find the essential 20% to get unstuck, and use sub-agents to find the best resources on the internet.
tags: [gornik, ai-methods, visualization, filtering, 80-20, resources, engineering, claude-code]
timestamp: 2026-11-04
---

# The Four AI Learning Methods

Back to [Gornik Learning Index](/gornik-learning/index.md) | [Brain Index](/index.md). Related: [Alvar Method Process](/alvar-method/process.md), [Huberman Self-Testing](/huberman-learning/self-testing-protocol.md).

> Source: Aleks Gornik, *How I use AI to learn engineering (anti brain rot studying)* (youtu.be/GPM5MgFEkpk). Transcript: [transcript.md](/gornik-learning/transcript.md).

## Core Thesis

> "AI gets me unstuck in minutes, and it's not by doing the work for me. It's by showing me the concepts in a visual manner, cutting out all the noise from the signal, and giving me those most important resources to learn ASAP."

AI is a **filter and accelerator**, not a replacement for your own thinking. The "anti brain rot" framing means: use AI to remove the logistics (finding resources, deciding what to read, getting past confusion) so your cognitive effort goes entirely into the material itself — exactly the principle Alvar calls "the system absorbs logistics" and Huberman calls "effort is the cornerstone."

## Method 1: Make It Visual

### The Problem
Engineering concepts are often abstract, presented in dense textbooks or terrible lecture recordings with bad audio. The gap between "reading about a concept" and "understanding it spatially" is huge.

### The Solution
Use AI (Claude Code, GPT Codex) to generate interactive 3D models, presentations, and dashboards that show concepts visually.

### Example 1: Mechanical Engineering — Robotics Project
Prompt:
> "Hey Claude, I want to build a robotics project, but I'm not really too sure where to start. Give me three ideas for an interactive presentation. Pitch them to me. Tell me why this one is the best one and why I should build it, and make a 3D model out of it so I can see it."

Result: AI generates a full presentation with a 3D model. You can spin it around, zoom in, zoom out, try it out.

### Example 2: Electrical Engineering — Hardware Wiring
Prompt:
> "Make me an interactive dashboard or presentation showing me how to wire the STM32 together to this other piece of hardware so I can take power readings."

Process: Take a photo of your hardware → AI designs an interactive tool showing how to wire this to that, step by step. Removes the confusion of terrible datasheets.

### Key Recommendation
> "I do recommend spending more of those expensive tokens on the better models because they are much better at the 3D world."

Better models (more expensive tokens) are significantly better at 3D visualization. This is worth the cost for visual learning.

### Why It Works (connecting to Huberman)
- Visual focus → mental focus (Huberman Super-Protocol Step 2)
- Showing objects (arrows, planes, machines) rather than screenshots of equations (Alvar `learn-visual` principle)
- The Alvar Method's `learn-visual` skill does this with self-reviewed SVGs; Gornik does it with interactive 3D

## Method 2: Filter Resources with AI

### The Problem
There are too many resources. A professor recommends 5 books, each 1,000 pages, one written in 1970. At work, there are too many research papers to read. You can't read everything.

### The Solution
Use AI as a filter to find exactly what you need from the mass of available material.

### The Prompt (University)
> "Here's my module/class syllabus, and here is the reading list. Which books and chapters from this book are actually worth my time considering my specific curriculum and the exams I'm going to sit? Which tutorial problems are worth doing?"

### The Process
1. **First pass**: Give AI the full book list + your syllabus → it tells you which book is most relevant
2. **Second pass**: Zoom in → ask which chapters of that book are the best → the 80/20
3. **Result**: Instead of reading 5,000 pages, you read 2-3 chapters

### The Prompt (Work / Research)
> "I found these five papers. I want to get a good update on this specific side of reinforcement learning. Which of these papers is the best to read? Which is the most legitimate?"

### The Result
> "I don't have to spend 2 hours reading papers when I can just spend 15 minutes."

### Why It Works (connecting to Alvar Method)
- This is what the Alvar Method's `learn-verify` skill does — fact-checking and source selection
- The Alvar Method's `research` subagent serves the same filtering role during the Plan phase
- "One interface, many sources" — AI aggregates all sources and delivers through one trusted interface

## Method 3: Find the Essential 20%

### The Problem
In any topic, you don't need to know everything. You don't even need to know half. You probably need to know 20% to get moving.

### The Origin Story
2023, hardware engineering intern, ChatGPT 3 just came out:
> "Could you explain this to me like I'm 5 years old?"

This was the seed of the approach. The "explain like I'm 5" prompt made complex topics accessible.

### The Story: Graph Neural Network Thesis
Aleks was doing his thesis in graph neural networks for simulating airway flow in the lungs. He got stuck. Deep learning has too much:

- Forward passes, activation functions, softmax, tanh, ReLU
- Inputs, outputs, matrices
- Different architectures: transformers, normal neural networks, graph neural networks
- Backpropagation

> "There's too much to study. There's too much to learn."

### The Breakthrough
He broke the problem down:
1. **What do I actually need?** Get a training loop going. See a few epochs. Feed it some training data.
2. **What do I focus on?** Just studying how to use PyTorch to run some data over a neural network.
3. **What don't I need yet?** Backpropagation (didn't know how it really works, but it was enough to get going).

### The Principle
> "Don't think that you need to study everything to get going. You do not, and it's going to cost you many, many hours if you think that's the case."

Use AI to figure out the 20% you need to know right now → get unstuck → go back and learn the rest if you want to.

### Why It Works (connecting to Huberman and Alvar)
- This is the Alvar Method's **Probe phase** — find the edge of understanding, start from `known`, path through `edge`, don't start in `unknown` with no ramp
- This is Huberman's **~15% error rate** — if you're at the edge, you're at the right difficulty
- This is the 80/20 principle applied to learning: identify the critical few concepts that unlock the rest

## Method 4: Find the Best Resources

### The Problem
Even after filtering, you need to find high-quality resources to learn from. Manual internet searching is slow and you might miss the best stuff.

### The Solution
Use AI sub-agents to crawl the internet and find the best resources.

### The Example: CAD Learning
Aleks knew very little about CAD but needed it for work:
1. Got AI to run sub-agents to scatter the whole internet
2. Found: NASA 3D models of Mars rovers, satellite files in .stl format — all free, open-sourced
3. Could literally build a Mars rover for free — download files, print it out

> "You searching through the internet for 10 hours is probably still going to be slower than 10 sub-agents crawling the internet, and they'll find you much better stuff if you just specify exactly what you're looking for."

### The Critical Warning
> "Just make sure you click on the links and double-check that everything is legit because AI does hallucinate still. Big FYI."

### Why It Works (connecting to Alvar Method)
- This is the Alvar Method's `research` subagent — fact-checking and source-finding
- The Alvar Method's `learn-verify` skill: "Do not cite a URL you did not open"
- Gornik's warning about hallucination is exactly why the Alvar Method engineers trust: verification before teaching

## Recap (from the video)

> "Make stuff visual, okay? Filter out all the BS which is on the internet. 80/20, okay? Figure out what the 20% that you need to know in your head right now to get stuff done. And finally, make sure when you are studying, you use the best resource to do so. And use AI to do these four things 'cause it's better at you than us, okay? It's better."

## The "Anti Brain Rot" Principle

The video title includes "anti brain rot" — the key distinction is:

| Brain rot (what NOT to do) | Anti brain rot (what TO do) |
|---|---|
| Let AI do the work for you | Use AI to show you concepts visually |
| Read everything the professor recommends | Use AI to filter to what matters |
| Try to learn everything before starting | Find the 20% that gets you unstuck |
| Manually search the internet for hours | Deploy sub-agents to find the best resources |
| Accept AI output without verification | Click links, double-check, verify everything |

The principle is the same as Alvar's "struggle stays in the material" and Huberman's "effort is the cornerstone": AI removes logistics, you keep the cognitive work.
