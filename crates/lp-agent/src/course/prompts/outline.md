Plan a course for this learner. Output JSON only, no code fence:
{"learner":"<3 to 5 lines: level, goal, known gaps>","must_fix":["each placement mistake"],"lessons":[{"id":"slug",
"title":"...","objective":"what they can do after","kind":"conceptual|procedural|definitional",
"depends_on":["ids of lessons this one needs, may be empty for the first lessons"],
"teach_notes":"<40 words: how to explain it to THIS learner>",
"key_facts":["up to 6 exact facts this lesson teaches, each correct"],"fixes":["placement mistakes it corrects"]}]}
Exactly {k} lessons in prerequisite order, starting at the edge of what the learner knows; `depends_on` lists
only earlier lessons' ids in this list. Every placement mistake must be in some lesson's fixes.
Learner: {topic}
