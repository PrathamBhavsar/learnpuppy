UNVERIFIED worker draft. It contains wrong citations. See brain/cheap-tutor/research.md.

### 1. How Production Learning Products Limit LLM Cost

Production learning products avoid sending full transcripts on every question by separating **curriculum selection and practice items** from **generative interactive features**:

* **Duolingo:** Standard practice items (exercises, sentences, quizzes) are **not generated on the fly by LLMs**. Duolingo serves pre-generated, human-curated exercises stored in large item banks. Session assembly is driven by **Birdbrain**, a statistical logistic model / IRT engine that matches user proficiency to item difficulty in milliseconds with zero LLM API calls ([Duolingo Birdbrain Blog](https://blog.duolingo.com/learning-how-to-help-you-learn-introducing-birdbrain/)). Duolingo uses LLMs upstream as an authoring accelerator (prompting models with structured templates before human instructional designers audit them) and restricts real-time LLM inference to high-tier subscription features (*Duolingo Max*), specifically **"Explain My Answer"** and **"Roleplay"** ([Duolingo Max Launch](https://blog.duolingo.com/duolingo-max/)).
* **Math Academy:** Operates with **zero real-time generative LLM calls** for core pedagogy. The entire system runs deterministically on a Directed Acyclic Graph (DAG) knowledge graph of thousands of granular skills. Problem templates with algorithmic parameter variations and spaced repetition via *Fractional Implicit Repetition (FIRe)* are evaluated with exact computational steps, not natural language models ([Math Academy FIRe](https://justinmath.com/fractional-implicit-repetition/)).
* **Khanmigo (Khan Academy):** Unlike Duolingo, Khanmigo runs real-time LLM calls (GPT-4) for its Socratic chat interactions ([OpenAI Khan Academy Case Study](https://openai.com/index/khan-academy/)). To absorb the token cost, Khan Academy gated the tool behind a paid subscription ($4/month or institutional district licensing) specifically to subsidize API inference tokens ([Khanmigo](https://www.khanacademy.org/khanmigo)).
* **Speak:** Employs real-time streaming LLM architecture because its core value proposition is open-ended spoken conversational roleplay. It routes audio through Whisper for speech-to-text, queries an LLM for conversational turns, and converts back via text-to-speech ([OpenAI Speak Case Study](https://openai.com/index/speak/)). Speak offsets inference costs through annual subscription pricing ($120–$180/year) and aggressive context pruning rather than pre-generation.
* **Quizlet:** For flashcards, Learn mode, and Test modes, items are retrieved directly from structured relational databases with no LLM calls. Quizlet introduced **Q-Chat** using OpenAI’s API for conversational tutoring against flashcard sets ([Quizlet Q-Chat Announcement](https://www.prnewswire.com/news-releases/quizlet-launches-q-chat-the-first-fully-adaptive-ai-tutor-built-on-openais-chatgpt-api-301759600.html)). High API consumption forced Quizlet to gate Q-Chat behind Quizlet Plus ($35.99/year), and Quizlet later phased out conversational Q-Chat in favor of deterministic study activities and one-off batch note generation (*Magic Notes*).

**Summary for LearnPuppy:** All scalable production apps pre-generate their core question banks and use statistical engines to schedule and serve them at $0 inference cost. Real-time LLMs are reserved strictly for conversational roleplay or deep error explanations.

---

### 2. Learner Modeling Without an LLM: Accuracy vs. Complexity with Few Observations

When choosing an offline learner model that must perform well with **sparse interactions (cold start / few observations per skill)**, the candidates compare as follows:

| Model | Complexity | Calibration on Sparse Data | Time Decay Native? |
| :--- | :--- | :--- | :--- |
| **Elo-based Learner Modeling / 1PL IRT** | **Lowest** ($O(1)$ update, 2 params: student ability $\theta$, item difficulty $d$) | **Best** (updates immediately; does not degenerate with 1–3 trials) | No (requires explicit time-decay add-on) |
| **Bayesian Knowledge Tracing (BKT)** | Low (4 params per skill: $p_{init}, p_{transit}, p_{slip}, p_{guess}$) | **Poor** (prone to parameter identifiability issues and degeneracy without dozens of observations) | No |
| **Half-Life Regression (HLR)** | Moderate (regularized regression over feature vectors) | Moderate (requires offline dataset to fit global weights) | Yes (predicts half-life $h$) |
| **FSRS (v4/v5)** | Moderate-High (17–19 parameters) | High on default parameters, but over-parameterized if fit on $<100$ reviews | Yes (tracks Difficulty, Stability, Retrievability) |

* **Empirical Winner for Sparse Skills:** **Elo-style rating / online logistic models.** Radek Pelánek’s comparative benchmark demonstrates that Elo rating variants perform on par with or outperform standard BKT in predictive accuracy ($AUC \approx 0.70\text{--}0.75$), while avoiding BKT's chronic identifiability failure modes when data per skill is limited ([Pelánek 2016, *Computers & Education*](https://doi.org/10.1016/j.compedu.2016.03.001); [Pelánek 2017, *UMUAI*](https://doi.org/10.1007/s11257-017-9193-2)).
* **For Spaced Memory Retention specifically:** **FSRS** is the benchmark leader in predicting forgetting curves for discrete memory items ([FSRS Benchmark](https://github.com/open-spaced-repetition/fsrs-benchmark)). However, FSRS should be run with its pre-trained global default weights; fitting its 19 parameters on few student observations causes severe overfitting.
* **Recommendation:** Use **Elo/1PL-IRT** for skill competency tracking and **FSRS (with default global weights)** for scheduling vocabulary/fact reviews.

---

### 3. Memory for LLM Tutors: Rolling Summaries vs. Structured State vs. Retrieval

Replacing raw transcripts with structured state or hybrid retrieval does **not** degrade pedagogical task success; it substantially **improves** accuracy by eliminating the "Lost in the Middle" attention degradation ([Liu et al. 2023](https://arxiv.org/abs/2307.03172)).

* **Rolling Summaries (Lossy Compression):** Research from MemGPT/Letta demonstrated that naive recursive or rolling summarization causes steep quality drops: fine-grained facts, exact timestamps, and specific learner misconceptions are systematically erased during compression passes ([Packer et al. 2023, *MemGPT*, arXiv:2310.08560](https://arxiv.org/abs/2310.08560)).
* **Structured State vs. Full Transcript:** Mem0’s benchmarks on conversational agent memory (evaluating on the *LoCoMo* and *LongMemEval* benchmarks) found that replacing raw context with an extracted, structured profile graph achieved **92.5% to 94% recall accuracy** while reducing prompt token consumption by **over 90%** (cutting contexts from $>25\text{k}$ tokens down to $1.6\text{k}\text{--}3\text{k}$ tokens) ([Mem0 Benchmark Paper, arXiv:2412.18619](https://arxiv.org/abs/2412.18619); [Mem0 Research Blog](https://mem0.ai/)).
* **Tutoring Domain Evidence:** Studies in intelligent tutoring systems show that LLM tutors parameterized by an explicit student state (e.g., JSON schema containing `{mastered_skills, active_misconceptions, target_concept, turn_count}`) maintain higher pedagogical coherence and lower instruction hallucination than tutors provided with raw 20-turn conversational logs ([St-Hilaire et al. 2024, arXiv:2404.14441](https://arxiv.org/abs/2404.14441)).

---

### 4. Model Cascades and Routing: Measured Savings and Quality Loss

Cascading and predictive routing allow small models to handle routine tutoring tasks while escalating complex diagnostic queries to frontier models:

* **FrugalGPT (Stanford):** Evaluated sequential cascading (calling small LLMs first, evaluating response confidence/agreement, and escalating to GPT-4 only when needed). On multi-choice QA (HEADQA) and reading comprehension, FrugalGPT achieved a **98.3% cost reduction** compared to GPT-4 alone while **matching 100% of GPT-4's accuracy** (and in some setups gaining +4% accuracy through ensemble scoring) ([Chen, Zaharia, & Zou 2023, arXiv:2305.05176](https://arxiv.org/abs/2305.05176)).
* **RouteLLM (LMSYS / UC Berkeley):** Tested embedding- and matrix-factorization-based routers to dispatch between a small model (e.g., Llama-3-8B) and a frontier model (GPT-4) on MT-Bench, Chatbot Arena, MMLU, and GSM8K. Results:
  * Achieved **85% cost reduction** on MT-Bench while maintaining **95% of GPT-4's performance benchmark** ([Ong et al. 2024, arXiv:2406.18665](https://arxiv.org/abs/2406.18665)).
  * Frontier models only needed to be called on 14–20% of turns to preserve human-evaluated quality.
* **Quality Trade-off:** Quality loss is $<2\text{--}5\%$ when classification thresholds are tuned to route simple answer checks and standard prompts to the small model, escalating exclusively on ambiguous user queries, detailed error corrections, or synthesis.

---

### 5. Language Learning Pedagogy for a 60-Minute Session

A research-grounded 60-minute session must balance cognitive load, input accessibility, and retrieval practice:

1. **Comprehensible Input (The 98% Lexical Threshold):** Krashen’s Input Hypothesis ($i+1$) dictates that acquisition requires understandable context ([Krashen 1982](http://www.sdkrashen.com/content/books/principles_and_practice.pdf)). Marcella Hu and Paul Nation demonstrated that learners require **98% lexical coverage** (knowing 49 out of 50 running words) for unassisted reading comprehension and contextual vocabulary inference; drop-offs to 95% or lower trigger severe comprehension breakdown ([Hu & Nation 2000](https://www.wgtn.ac.nz/lals/resources/paul-nations-resources/paul-nations-publications/publications/documents/2000-Hu-Vocabulary-density.pdf)).
2. **Retrieval Practice (The Testing Effect):** Active recall dramatically outperforms restudying for retention. Roediger & Karpicke showed that repeated retrieval produces up to **400% higher delayed recall** at 1 week compared to passive re-reading, even without additional feedback ([Roediger & Karpicke 2006, *Psychological Science*](https://doi.org/10.1111/j.1745-6916.2006.00012.x)).
3. **Interleaving vs. Blocking:** Mixing distinct but related concepts (e.g., alternating between past tense forms vs. drilling a single verb form) produces superior delayed transfer and category discrimination ([Rohrer & Taylor 2007](https://doi.org/10.1007/s11251-007-9015-8); [Pan et al. 2019, *Journal of Educational Psychology*](https://doi.org/10.1037/edu0000314)).
4. **Ratio of New to Review:** Pedagogical guidelines for vocabulary acquisition (Nation 2006; Pimsleur method) prescribe approximately **10%–20% new items to 80%–90% retrieval review** per session. Introducing more than 5–10 new target lexemes in a 60-minute session overwhelms working memory consolidation.
5. **Cognitive Fatigue Timeline (Vigilance Decrement):** Sustained attention tasks demonstrate a reliable performance and reaction decrement after **20 to 30 continuous minutes** due to executive resource depletion ([Mackworth 1948](https://doi.org/10.1080/17470214808416738); [Langner & Eickhoff 2013, *Cerebral Cortex*](https://doi.org/10.1093/cercor/bhs314)). Sessions of 60 minutes must be divided into two 25-minute intervals separated by a 5-minute cognitive reset (Pomodoro structure) to maintain encoding fidelity.

---

### 6. Automatic Quality Gates for Generated MCQs (Rule-Based Detection)

Flawed items introduce *construct-irrelevant variance* (measuring test-wiseness rather than knowledge). Based on the taxonomy by Haladyna, Downing, & Rodriguez ([Haladyna et al. 2002, *Applied Measurement in Education*](https://doi.org/10.1207/S15324818AME1503_5)), the following flaws can be detected via **deterministic, non-LLM regex and rule checks**:

1. **Option Length Clue (The "Longest Option is Correct" Cue):**
   * *Rule:* The correct key is significantly longer than all distractors.
   * *Detection:* Flag any item where `len(correct_option) > 1.5 * mean(len(distractors))` and is the longest option.
2. **Grammatical Association / Stem-Option Concord:**
   * *Rule:* Stem ends with indefinite article ("a" or "an") or singular/plural verb matching only the key.
   * *Detection:* Regex checks if stem ends with `\ba\b` while options begin with vowels (or stem ends with `\ban\b` while options begin with consonants).
3. **Absolute Words (Specific Determiners):**
   * *Rule:* Distractors using absolutes are recognized as false by test-wise students.
   * *Detection:* Regex scan across options for `\b(always|never|completely|impossible|none|all|every)\b`.
4. **"All of the Above" / "None of the Above":**
   * *Rule:* Weakens discrimination and allows partial guessing.
   * *Detection:* Exact substring match on `\b(all|none) of the above\b`.
5. **Overlapping / Subset Options:**
   * *Rule:* Numerical or categorical intervals that overlap.
   * *Detection:* Set intersection or numeric interval overlap parsing on options (e.g., Option A: $0\text{--}5$, Option B: $5\text{--}10$).
6. **Word Repetition / Clang Clues:**
   * *Rule:* A salient word from the question stem appears only in the correct option.
   * *Detection:* Set intersection of non-stopword tokens between stem and option; flag if `tokens(stem) ∩ tokens(key) ≠ ∅` while `tokens(stem) ∩ tokens(distractors) = ∅`.
7. **Unformatted Negative Stems:**
   * *Rule:* Negative terms (*NOT*, *EXCEPT*) cause confusion unless visually capitalized.
   * *Detection:* Regex search for lowercase `\b(not|except|least)\b` in stem without bold/uppercase markup.
