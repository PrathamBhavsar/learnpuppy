Write 5 placement questions that find out what this learner already knows about their goal, from easy to hard. Mix mcq and recall (at least 2 of each). Output JSON only, no code fence:
{"items":[{"type":"mcq","q":"...","options":["a","b","c","d"],"answer":"<exact option text>","why":"<one sentence>"},
{"type":"recall","q":"...","answer":"<model answer>","accept":["other correct answers"],"why":"<one sentence>"}]}
Options similar in length and all plausible. Never put the answer in the question. `why` teaches the topic to the learner: never mention the question, the test or the learner.
Learner goal: {topic}
