Write one lesson. Output JSON only, no code fence:
{"explain":"<under 120 words, with one concrete example>","items":[{"type":"mcq","q":"...","options":["a","b","c","d"],
"answer":"<exact option text>","why":"<one sentence>","tests":0},{"type":"recall","q":"...","answer":"<model answer>",
"accept":["other correct answers"],"why":"...","tests":1}]}
Exactly {n} items, 4 mcq then 2 recall. `tests` is the index of the key fact the item tests. Explain must teach every
key fact. Options similar in length and all plausible. Do not repeat any used question. Assume earlier lessons were
studied. Only make comparisons with the learner's background that you are sure are correct.
Learner: {learner}
Lesson: {lesson}
Used questions: {used}
