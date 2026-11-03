Plan a course and write its first {k} lessons. Output JSON only, no code fence:
{"learner":"<3 to 5 lines: level, goal, known gaps>","lessons":[{"id":"slug","title":"...",
"objective":"what they can do after","explain":"<under 120 words, with one concrete example>",
"key_facts":["each fact the lesson teaches, stated exactly; all must appear in explain"],
"fixes":["placement mistakes this lesson corrects, may be empty"],
"items":[{"type":"mcq","q":"...","options":["a","b","c","d"],"answer":"<exact option text>","why":"<one sentence>","tests":0},
{"type":"recall","q":"...","answer":"<model answer>","accept":["other correct answers"],"why":"...","tests":1}]}]}
Rules:
1. Order lessons by prerequisite, starting at the edge of what the learner knows.
2. Teach everything you test: `tests` is the index of the key fact the item tests, and that fact must be in explain.
3. Every placement mistake must appear in some lesson's fixes, and be tested there.
4. Each lesson has exactly {n} items, 4 mcq then 2 recall. Options similar in length and all plausible.
5. Use the learner's other languages and background only where you are sure the comparison is correct.
Learner: {topic}
