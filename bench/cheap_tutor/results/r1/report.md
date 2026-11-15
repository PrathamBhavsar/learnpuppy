# Results

95 calls, 0 failed.

## Per-step approaches, projected to a full session

| approach | topic | model | real steps | tokens in, step 1 | tokens in, last | projected 60-step input | failed |
|---|---|---|---|---|---|---|---|
| baseline | spanish | haiku | 10 | 1675 | 4321 | 620880 | 0 |
| baseline | spanish | flash-low | 10 | 11982 | 13579 | 1032996 | 0 |
| state | spanish | haiku | 10 | 878 | 1006 | 77853 | 0 |
| state | spanish | flash-low | 10 | 11414 | 11500 | 701753 | 0 |

## Pack approaches, projected to a full session

Input = skeleton + one pack call per lesson + one grading call per lesson. Strong calls are counted separately.

| variant | topic | strong in | cheap in | cheap out | total in | packs parsed | items passing gates | flaws |
|---|---|---|---|---|---|---|---|---|
| strong-skeleton+haiku | spanish | 904 | 24253 | 31253 | 25157 | 3/3 | 17/18 | {'answer is conspicuously longest': 1} |
| strong-skeleton+flash-low | spanish | 904 | 232176 | 5700 | 233080 | 3/3 | 17/18 | {'needs 4 distinct options': 1} |
| strong-skeleton+qwen | spanish | 904 | 6460 | 4670 | 7364 | 1/3 | 6/6 | - |
| cheap-skeleton+haiku | spanish | 0 | 24782 | 27280 | 24782 | 3/3 | 17/18 | {'needs 4 distinct options': 1} |
| cheap-skeleton+flash-low | spanish | 0 | 243117 | 5156 | 243117 | 3/3 | 17/18 | {'answer is conspicuously longest': 1} |
| cheap-skeleton+qwen | spanish | 0 | 5816 | 5760 | 5816 | 3/3 | 16/16 | - |
| strong-writes-all | spanish | 3020 | 0 | 0 | 3020 | - | - | out for 3 lessons: 5283 |
| strong-skeleton+haiku | rust | 880 | 23856 | 24616 | 24736 | 3/3 | 18/18 | - |
| strong-skeleton+flash-low | rust | 880 | 233096 | 7666 | 233976 | 1/3 | 6/6 | - |
| strong-skeleton+qwen | rust | 880 | 8346 | 7503 | 9226 | 3/3 | 18/18 | - |
| cheap-skeleton+haiku | rust | 0 | 24991 | 27403 | 24991 | 3/3 | 18/18 | - |
| cheap-skeleton+flash-low | rust | 0 | 212742 | 7086 | 212742 | 3/3 | 18/18 | - |
| cheap-skeleton+qwen | rust | 0 | 5634 | 6240 | 5634 | 2/3 | 12/12 | - |
| strong-writes-all | rust | 2940 | 0 | 0 | 2940 | - | - | out for 3 lessons: 4098 |

## Blind judge (1 to 5)

| topic | variant | accuracy | teaching | level | variety | errors | note |
|---|---|---|---|---|---|---|---|
| spanish | cheap-skeleton+flash-low | 4 | 3 | 5 | 3 | L1 explanation: 'días' is said to refer to morning, but it literally means 'days'; only the whole phrase 'Buenos días' is a morning greeting. | The level and workplace formality fit well, but items repeat the same clock-time and 'mi llamo' checks, and the mañana note never says that mañana means tomorrow. |
| spanish | cheap-skeleton+haiku | 5 | 4 | 5 | 4 | - | This is a clean beginner sequence with a helpful Hindi and Gujarati formality link, but it ignores the learner's probe errors, misses Peru's ll and softer j, and tests trabajo stress twice. |
| spanish | cheap-skeleton+qwen | 1 | 1 | 1 | 3 | L0 Q1: 'Soy Carlos' is a natural way to introduce yourself, so the distractor is only wrong under a strictly literal reading of the stem.; L0 Q3: Mucho gusto is called a formal greeting used first with a boss, but it means 'nice to meet you', is not specifically formal, and does not have to come fir | This has multiple factual and item-construction errors, relies on grammar jargon, tests untaught material, and rushes a true beginner into ser versus estar instead of addressing the probe errors. |
| spanish | strong-skeleton+flash-low | 4 | 4 | 4 | 3 | L1 explanation and recall: 'Mi nombre es' is presented as matching 'Mera naam hai' word for word, but Hindi puts the verb last ('Mera naam Alex hai') while Spanish puts it before the name ('Mi nombre es Alex'). | The dental त/द link and name patterns are useful, but the capitalization item is low value and mañana is glossed without being tested or contrasted with today. |
| spanish | strong-skeleton+haiku | 5 | 5 | 5 | 4 | - | This is accurate, well suited to a Peru-focused workplace learner, and fixes the mañana confusion, though trabajo stress and ñ are each tested twice and 'mi llamo' is never corrected. |
| spanish | strong-skeleton+qwen | 4 | 3 | 4 | 3 | Recall 1: The why claims 'mi nombre es Alex' matches the structure of 'मेरा नाम Alex है', but the Hindi verb comes last while the Spanish verb comes before the name.; Q1: The stem has a punctuation error, with no closing quotation mark after 'My name is Alex?'. | It targets 'mi llamo' directly but provides only one lesson, tests the untaught ¿Cómo te llamas? and Igualmente, duplicates 'Me llamo Alex', and mentions mañana only in passing. |
| spanish | strong-writes-all | 5 | 4 | 5 | 5 | - | This is the most personalized variant, directly fixing both written errors with useful Hindi and Peruvian links, but dónde is tested without being glossed and the mañana confusion and usted are not covered. |
| rust | cheap-skeleton+flash-low | 3 | 4 | 3 | 4 | Lesson 1, explanation: 'let s2 = s1 moves heap data to s2' is wrong because the heap data stays put; only the String's pointer, length and capacity are copied and ownership is transferred.; Lesson 0, explanation: 'Types exist so the compiler can determine exact memory allocation upfront on the stack | Reaches ownership quickly and covers what it tests, but unexplained jargon such as LLVM and the stack suits a first-time compiled-language learner poorly, and the move explanation plants a misconception. |
| rust | cheap-skeleton+haiku | 4 | 5 | 4 | 3 | Lesson 0, explanation: 'Python reads your script and runs it line by line' is simplified because CPython first compiles the whole file to bytecode, so syntax errors appear before execution.; Lesson 1, MCQ 1: the wording 'when the program runs x = 6' is misleading because the code never compiles and  | Explanations align tightly with the items and suit a beginner to compilation, but the recalls repeat the MCQ concepts, some whys describe the test rather than teach, and ownership is not yet reached. |
| rust | cheap-skeleton+qwen | 2 | 3 | 3 | 4 | Lesson 1, MCQ 1: two options are valid because 'let t = (5, "hello");' also compiles, yet only the annotated form is keyed.; Lesson 1, MCQ 1 why: 'Tuples require type annotations for each element' is false because tuple types can be inferred.; Lesson 1, MCQ 3: 'Panics at runtime' is not reliably cor | This syntax-first sequence has several incorrect claims and keys, tests facts the explanation omits such as char size and bounds checks, and neglects both compilation basics and ownership. |
| rust | strong-skeleton+flash-low | 4 | 5 | 4 | 4 | Lesson 0, explanation: 'python app.py interprets source code directly' is simplified because CPython compiles source to bytecode first.; Lesson 0, MCQ 2: 'only the standalone binary' is overstated because the binary must match the target platform and may dynamically link system libraries such as lib | Clear Python-to-Cargo mapping that fully prepares the learner for the items, but it stays on tooling and never reaches language semantics or the learner's ownership gap. |
| rust | strong-skeleton+haiku | 4 | 5 | 5 | 4 | Lesson 0, recall 2: 'the two files that cargo new creates' is not strictly true because it normally also creates .gitignore and initializes a Git repository.; Lesson 0, explanation: 'adding a semicolon makes the function return ()' is imprecise because in a function declared to return i32, the body  | Well-paced, accurate coverage moves from Cargo and functions to moves, drops and clone with a direct Python contrast, and only minor overlap among the move items. |
| rust | strong-skeleton+qwen | 3 | 3 | 4 | 4 | Lesson 0, MCQ 4: 'Release builds optimize code and are smaller' is also defensible, so the item effectively has two correct answers.; Lesson 1, MCQ 1: the keyed answer says you 'must' shadow, but the direct result of reassignment is a compile error and 'let mut' is another valid fix.; Lesson 1, MCQ  | The Python analogies are useful, but debug and release builds, fmt, clippy, shadowing and integer types are tested without being taught, and the shadowing item is flawed. |
| rust | strong-writes-all | 5 | 4 | 5 | 5 | - | The best fit for this learner, with accurate Python contrasts and direct use of the probe result, though cargo run, --release, clone() and moves into functions are tested without being explained. |

## Call time

| provider/model | calls | mean seconds | failed |
|---|---|---|---|
| agy gemini-3.8-flash-low | 35 | 22.4 | 0 |
| claude claude-haiku-5-5 | 35 | 11.7 | 0 |
| claude claude-opus-5-5 | 10 | 64.1 | 0 |
| kiro qwen3-coder-next | 15 | 15.2 | 0 |
