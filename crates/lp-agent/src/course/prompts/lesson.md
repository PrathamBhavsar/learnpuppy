Write one lesson. Output JSON only, no code fence:
{"explain":"<under 120 words, with one concrete example>","hook":"<one line, why this matters>",
"aside":"<optional related-but-not-central note, or omit>",
"predict":{"q":"...","options":["a","b","c","d"]},
"angles":["1 to 2 alternate explanations, different wording or example, for a no-reveal re-teach"],
"rungs":[{"kind":"full","text":"..."},{"kind":"blanked","text":"..."},{"kind":"solo","text":"..."}],
"claims":["each checkable factual claim this lesson makes, stated plainly, or empty"],
"items":[{"type":"mcq","q":"...","options":["a","b","c","d"],"answer":"<exact option text>","why":"<one sentence>",
"tests":0,"level":2,"misconceptions":["per option, the belief implied by choosing it; same length as options, empty string for the right one"]},
{"type":"recall","q":"...","answer":"<model answer>","accept":["other correct answers"],"why":"...","tests":1,"level":2,
"must_name":["terms the answer must name to confirm by code alone, or empty if too open-ended"]}]}
Omit "predict" on a pure definition. Leave "rungs" empty unless `kind` is `procedural`. Exactly {n} items, 4
mcq then 2 recall, levels 1 to 3 mixed. `tests` is the index of the key fact the item tests. Explain must
teach every key fact. Options similar in length and all plausible. Do not repeat any used question. Assume
earlier lessons were studied. Only make comparisons with the learner's background that you are sure are
correct.
Learner: {learner}
Lesson: {lesson}
Used questions: {used}
