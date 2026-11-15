Fix only the broken lessons below. Output JSON only, no code fence: {"lessons":[<each fixed lesson, same shape, same id>]}
A lesson has: id, title, objective, explain (under 120 words, one concrete example), key_facts, fixes, and exactly {n} items (4 mcq then 2 recall;
mcq has "options" with 4 distinct plausible options and "answer" equal to one option's exact text; recall has "answer" and "accept"; every item has "why" and "tests", the index of the key fact it tests).
Return every lesson listed under Broken lessons and nothing else. Each failed rule must be fixed.
Failed rules:
{rules}
Learner: {topic}
Broken lessons:
{lessons}
