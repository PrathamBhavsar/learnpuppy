Plan a course and write its first {k} lessons. Output JSON only, no code fence:
{"learner":"<3 to 5 lines: level, goal, known gaps>","lessons":[{"id":"slug","title":"...",
"objective":"what they can do after","kind":"conceptual|procedural|definitional",
"depends_on":["ids of earlier lessons in this list that this one needs, may be empty"],
"hook":"<one line, why this matters>","aside":"<optional related-but-not-central note, or omit>",
"explain":"<under 120 words, with one concrete example>",
"key_facts":["each fact the lesson teaches, stated exactly; all must appear in explain"],
"fixes":["placement mistakes this lesson corrects, may be empty"],
"predict":{"q":"...","options":["a","b","c","d"]},
"angles":["1 to 2 alternate explanations, different wording or example, for a no-reveal re-teach"],
"rungs":[{"kind":"full","text":"..."},{"kind":"blanked","text":"..."},{"kind":"solo","text":"..."}],
"claims":["each checkable factual claim this lesson makes, stated plainly, or empty"],
"items":[{"type":"mcq","q":"...","options":["a","b","c","d"],"answer":"<exact option text>","why":"<one sentence>",
"tests":0,"level":2,"misconceptions":["per option, the belief implied by choosing it; same length as options, empty string for the right one"]},
{"type":"recall","q":"...","answer":"<model answer>","accept":["other correct answers"],"why":"...","tests":1,"level":2,
"must_name":["terms the answer must name to confirm by code alone, or empty if too open-ended"]}]}]}
Rules:
1. Order lessons by prerequisite, starting at the edge of what the learner knows; `depends_on` lists only
   earlier lessons' ids in this list.
2. Teach everything you test: `tests` is the index of the key fact the item tests, and that fact must be in explain.
3. Every placement mistake must appear in some lesson's fixes, and be tested there.
4. Each lesson has exactly {n} items, 4 mcq then 2 recall, levels 1 to 3 mixed. Options similar in length and all plausible.
5. Use the learner's other languages and background only where you are sure the comparison is correct.
6. Omit "predict" on a pure definition. Leave "rungs" empty unless `kind` is `procedural`. "aside" may be omitted.
7. An angle re-teaches after a miss, so it must NOT state the answer to any item of its lesson. Use a different example or
   analogy and leave the final step to the learner; never write an item's answer text inside an angle.
Learner: {topic}
