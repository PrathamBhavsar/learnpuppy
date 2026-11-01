import type { InfoContent } from "@/components/app/info/info"

// Plain-words explanations for the Learn screen. Each entry feeds both its InfoTip and the screen guide.
export const classroomInfo = {
  learn: {
    title: "Learn",
    short: "Your lessons: each chat is one thing you are learning.",
    body: [
      "Start a chat, name what you want to learn (for example 'Spanish for travel' or 'the past tense in Spanish'), and an AI tutor teaches you in a back-and-forth conversation.",
      "Your chats are listed on the left. Click one to continue where you stopped. Right-click a chat, or use its menu, to rename or delete it.",
    ],
  },
  newChat: {
    title: "New chat",
    short: "Start a new lesson, either Teach or Practice.",
    body: [
      "Pick Teach to learn something from scratch, or Practice to drill something you have already met.",
      "Then write what you want to learn. You can also tick ideas from your knowledge graph so the lesson focuses on them.",
    ],
  },
  kind: {
    title: "Teach or Practice",
    short: "Teach explains a topic; Practice gives you problems.",
    body: [
      "Teach: the tutor first asks a few short questions to see what you already know, then writes a lesson plan, then teaches it step by step, quizzing you on the way.",
      "Practice: the tutor skips the lecture and gives you problems to solve, with hints if you get stuck. Best for things you have already learned, such as the ideas picked from your graph.",
    ],
  },
  topic: {
    title: "What to write as a topic",
    short: "A subject you want to learn, in your own words.",
    body: [
      "Be specific enough that a teacher knows where to start, but short. Good examples: 'Spanish for ordering food and asking directions', 'Spanish: ser versus estar', 'present tense of regular -ar verbs'.",
      "In Practice you can leave it empty if you picked ideas from the graph instead.",
    ],
  },
  nodes: {
    title: "Use nodes from the graph",
    short: "Base the lesson on ideas you already have.",
    body: [
      "Your knowledge graph holds ideas you met in earlier lessons or sessions. Picking some here makes the tutor focus on exactly them, which is perfect for revising.",
    ],
  },
  roles: {
    title: "What will run",
    short: "Which AI models will play each role in this chat.",
    body: [
      "A chat uses several AI roles, for example one that asks questions, one that teaches and one that checks answers. This box shows which model and account each will use. You can change them in Settings.",
    ],
  },
  phases: {
    title: "Lesson phases",
    short: "Probe, then Plan, then Teach (or Practice).",
    body: [
      "Probe: a few quick questions so the tutor learns what you already know. Guessing is fine. The counter shows how many have been asked out of the most it will ask.",
      "Plan: the tutor drafts a lesson plan, a small map of steps.",
      "Teach: the tutor works through the plan with you. In a Practice chat this last step is Practice, with problems instead of lectures.",
      "A check mark means a phase is finished; the ring marks where you are now.",
    ],
  },
  probe: {
    title: "Probe question",
    short: "A quick question to find out what you already know.",
    body: [
      "There are no marks for these. The tutor uses your answers (and your confidence) to decide what to skip and what to explain properly. A guess is fine.",
    ],
  },
  plan: {
    title: "Lesson plan",
    short: "The steps of this lesson, as a small map.",
    body: [
      "Each circle is a step. Arrows show which step builds on which. The current step is highlighted, finished ones are marked visited, and the rest are upcoming.",
      "The plan appears after the probe questions. You do not edit it; the tutor updates it as the lesson goes.",
    ],
    tips: [
      "Use the zoom buttons or the mouse wheel for big plans.",
      "'Fit plan' brings everything back into view.",
    ],
  },
  jobs: {
    title: "Jobs",
    short: "Background work the tutor does while you chat.",
    body: [
      "Sometimes the tutor starts a helper that works on its own: researching a topic on the web, reading a PDF you attached, drawing a diagram or building a small demo.",
      "Each job shows whether it is running, done or failed. Open a finished job to see what it made, or abort one that is still running.",
    ],
  },
  quiz: {
    title: "Quiz",
    short: "A multiple-choice question about what was just taught.",
    body: [
      "Pick an answer, set how sure you are, and submit. You get a Correct or Not quite right away, with an explanation.",
    ],
    tips: [
      "Press 1-9, or a-d for the first four, to pick an option, and Enter to submit.",
    ],
  },
  recall: {
    title: "Recall",
    short: "Say what you remember, without looking.",
    body: [
      "Recalling something from memory is what makes it stick. Type what you remember. If nothing comes, press 'I don't know'; that is useful information for the tutor.",
    ],
  },
  explain: {
    title: "Explain",
    short: "Teach it back in your own words.",
    body: [
      "Explaining an idea as if to a friend shows whether you really understand it. Around 12 words or more works well. The tutor then tells you what was right and what was missing.",
    ],
    tips: ["Enter sends, Shift+Enter starts a new line."],
  },
  problem: {
    title: "Problem",
    short: "A practice exercise to solve in your own words.",
    body: [
      "Write your answer and submit. The tag above the statement says the kind of exercise: recall (remember it), discrimination (tell similar things apart, like ser and estar) or composition (put pieces together to make something new).",
      "Stuck? Ask for a hint.",
    ],
  },
  question: {
    title: "Question from the tutor",
    short: "The tutor needs an answer from you before it can go on.",
    body: ["Type or pick your answer and submit it."],
  },
  confidence: {
    title: "How sure are you?",
    short: "Tell us how confident you are, from guessing to certain.",
    body: [
      "Confidence is not about being right. It lets the app tell apart a lucky guess from real knowledge, and a confident mistake from an honest 'not sure'. That makes your reviews and lessons smarter.",
      "Be honest: a 'guessing' answer tells the tutor to go back over that part, even when it happened to be right.",
    ],
  },
  idk: {
    title: "I don't know",
    short: "An honest 'no idea' is better than a guess.",
    body: [
      "It counts as not knowing yet, with no penalty. The tutor will then teach that part instead of assuming you have it.",
    ],
  },
  hints: {
    title: "Hint ladder",
    short: "Hints that start gentle and get more direct.",
    body: [
      "Each press of 'Give me a hint' reveals the next rung of the ladder: first a nudge, later something closer to the answer. Using fewer hints is better for your result, but a hint is always better than giving up.",
    ],
  },
  verdict: {
    title: "Answer check",
    short: "Whether your last answer was right, and why.",
    body: [
      "'Correct' or 'Not quite' comes with a short explanation. A miss is not a failure: it is exactly what the app uses to decide what to practise next.",
    ],
  },
  problemResult: {
    title: "Problem result",
    short: "How the problem went, and what to work on.",
    body: [
      "Outcome says solved, solved with hints, missed or gave up. 'Miss' names the kind of mistake. 'Rung' is how many hints you used. 'Retest later' means the app will bring this problem back.",
    ],
  },
  composer: {
    title: "Message box",
    short: "Write to the tutor.",
    body: [
      "Ask a question, or reply to the tutor. While a question is waiting for you, this box is replaced by the answer area.",
    ],
    tips: ["Enter sends, Shift+Enter starts a new line."],
  },
  steer: {
    title: "Steer",
    short: "Give the tutor guidance while it is still replying.",
    body: [
      "While the tutor is writing, normal messages wait. Turn on Steer to send a short instruction right away, such as 'slower please' or 'give me an example in Spanish'.",
    ],
  },
  stop: {
    title: "Stop",
    short: "Cancel the tutor's current reply.",
    body: ["Stops what the tutor is doing so you can say something else."],
    tips: ["Press Escape to stop."],
  },
  pdf: {
    title: "Attach a PDF",
    short: "Let the tutor read a document for you.",
    body: [
      "Upload a PDF, such as a textbook chapter or a worksheet. A PDF job reads it in the background and the tutor can use it in the lesson.",
    ],
  },
  thinking: {
    title: "Thinking",
    short: "The tutor's reasoning, before it answers.",
    body: [
      "Click 'Thinking' or 'Thought process' to read how the tutor reasoned. You never need to; it is only there if you are curious.",
    ],
  },
  tools: {
    title: "Tool notices",
    short: "Small tags such as 'Quiz asked' or 'Plan drafted'.",
    body: [
      "They show what the tutor just did behind the scenes: asked a probe, quiz, recall or explain question, drafted or updated the plan, or started a research, PDF, diagram or demo job.",
    ],
  },
} satisfies Record<string, InfoContent>

export const classroomGuide = {
  title: "About Learn",
  summary:
    "Learn is where you study. An AI tutor finds out what you know, makes a plan, then teaches and quizzes you.",
  parts: [
    classroomInfo.learn,
    classroomInfo.newChat,
    classroomInfo.kind,
    classroomInfo.topic,
    classroomInfo.nodes,
    classroomInfo.roles,
    classroomInfo.phases,
    classroomInfo.probe,
    classroomInfo.plan,
    classroomInfo.jobs,
    classroomInfo.quiz,
    classroomInfo.recall,
    classroomInfo.explain,
    classroomInfo.problem,
    classroomInfo.confidence,
    classroomInfo.idk,
    classroomInfo.hints,
    classroomInfo.verdict,
    classroomInfo.problemResult,
    classroomInfo.composer,
    classroomInfo.steer,
    classroomInfo.stop,
    classroomInfo.pdf,
    classroomInfo.thinking,
    classroomInfo.tools,
  ] satisfies InfoContent[],
}

/** Which tip explains the question type currently in the dock. */
export function questionInfo(tool: string): InfoContent {
  switch (tool) {
    case "probe":
      return classroomInfo.probe
    case "quiz":
      return classroomInfo.quiz
    case "recall":
      return classroomInfo.recall
    case "explain":
      return classroomInfo.explain
    case "problem":
      return classroomInfo.problem
    default:
      return classroomInfo.question
  }
}
