Write a strand probe for this learner's goal: 3 independent strands, each a distinct sub-skill or
sub-topic inside the goal, with one item at level 1 (easiest), one at level 2 (middle) and one at level 3
(hardest) testing that same strand at rising difficulty. Output JSON only, no code fence:
{"strands":[{"name":"...","items":[
{"type":"mcq","q":"...","options":["a","b","c","d"],"answer":"<exact option text>","why":"<one sentence>","level":1},
{"type":"recall","q":"...","answer":"<model answer>","accept":["other correct answers"],"why":"...","level":2},
{"type":"mcq","q":"...","options":["a","b","c","d"],"answer":"<exact option text>","why":"<one sentence>","level":3}
]}]}
Exactly 3 strands, each with exactly 3 items at levels 1, 2 and 3 (any order), mixing mcq and recall. No
teaching, no explanation text. Options similar in length and all plausible. Never put the answer in the
question.
Learner goal: {topic}
