import type { InfoContent } from "@/components/app/info/info"

// Plain-words explanations for the Review screens. Each entry feeds both its InfoTip and the screen guide.
export const reviewInfo = {
  review: {
    title: "Review",
    short: "Short daily quizzes that keep what you learned from fading.",
    body: [
      "Each card is a small multiple-choice question about something you learned. You answer it, see whether you were right, then say how easy it felt.",
      "The app uses your answer to decide when to show you that card again. This is called spaced repetition: you see a card just before you would forget it, so each review sticks longer than the last. A few minutes a day is enough.",
    ],
  },
  spaced: {
    title: "Spaced repetition and FSRS",
    short: "Cards come back at growing gaps, timed to your memory.",
    body: [
      "Forgetting is normal and predictable. If you review something right when it is about to fade, the memory gets stronger and lasts longer. Review it too early and you waste time; too late and you have to relearn it.",
      "FSRS is the scheduling formula that makes this prediction for every card, using how you rated it before. You never have to set anything up. It decides the due date.",
    ],
  },
  due: {
    title: "Due now",
    short: "Cards that are ready to be reviewed right now.",
    body: [
      "A card becomes due when the scheduler thinks it is time to see it again. Clearing your due cards each day is the whole habit.",
    ],
  },
  newToday: {
    title: "New today",
    short: "Cards you saw for the very first time today.",
    body: ["Brand-new cards you reviewed for the first time today."],
  },
  reviewed: {
    title: "Reviewed today",
    short: "How many cards you have answered today.",
    body: ["Every card you rated today, new or not."],
  },
  streak: {
    title: "Streak",
    short: "Days in a row on which you reviewed at least one card.",
    body: [
      "It counts back from today. If you have not reviewed yet today, yesterday still counts, so you have until the end of the day to keep it going. The 'd' means days.",
    ],
  },
  retention: {
    title: "Retention (30 days)",
    short: "How often you remembered cards you had seen before.",
    body: [
      "Of the cards you reviewed again in the last 30 days, this is the share you did not rate 'Again'. Higher means your memory is holding up. It shows a dash until there is enough history.",
    ],
  },
  forecast: {
    title: "14-day forecast",
    short: "How many cards will come due on each of the next 14 days.",
    body: [
      "Each bar is one day, starting today. Tall bars are busy days. Hover a bar for the date and the exact number. Click the title to fold it away.",
    ],
  },
  controls: {
    title: "Session settings",
    short: "How many cards to do now, and in what order.",
    body: [
      "Cards per session caps how long a sitting is; the rest stay due for next time.",
      "Shuffle sources mixes cards from different topics so you do not get ten of the same kind in a row. Mixing makes you work harder, and that is good for memory.",
    ],
  },
  card: {
    title: "A review card",
    short: "One question with four possible answers.",
    body: [
      "Read the question and pick one of four answers. The small label above it tells you the card's state: New (never seen), Learning, Review (a graduated card) or Relearning (one you forgot and are learning again).",
      "'Source decision' opens the work decision the card came from, if it was made from your coding sessions.",
    ],
    tips: [
      "Press 1-4 or a-d to pick an answer.",
      "Esc leaves the session, after asking you to confirm.",
    ],
  },
  progress: {
    title: "Session progress",
    short: "Which card you are on and how far through the session.",
    body: [
      "'Card 3 of 20' and the bar move forward each time you rate a card.",
    ],
  },
  verdict: {
    title: "Right or wrong",
    short: "Whether your answer was correct, with the reason.",
    body: [
      "After you pick, the app tells you right away, shows the correct answer and explains why. Being wrong is useful: it tells the scheduler to bring the card back sooner.",
    ],
  },
  rating: {
    title: "How well did you recall it?",
    short: "Again, Hard, Good or Easy: your honest feeling about the card.",
    body: [
      "Again: you did not remember it. Hard: you got there with effort. Good: you remembered it normally. Easy: it was instant.",
      "Under each choice is when the card would come back if you chose it, for example '10m', '3d' or '2w'. Better ratings mean longer gaps.",
      "The outlined choice is the app's suggestion, based on whether you were right. You may overrule it: if you got it right by luck, pick Hard or Again.",
    ],
    tips: [
      "Press 1 to 4 to rate, or Space to accept the suggestion.",
      "Be honest. An inflated rating only means you meet the card too late.",
    ],
  },
  summary: {
    title: "Session summary",
    short: "How the session went and when to come back.",
    body: [
      "Ratings counts how many of each rating you gave. Accuracy is the share of questions you answered correctly. Time is how long you took. Next review is when your next card is due.",
      "Mastery shows, per topic, how much of what you reviewed you rated Good or Easy.",
    ],
  },
  cardsBrowser: {
    title: "Cards browser",
    short: "Every card in this workspace, searchable.",
    body: [
      "Use the Cards tab to look through all your cards, search their questions, filter by state, or show only the ones due. Click a card for its details and history.",
    ],
  },
  cardState: {
    title: "Card state and details",
    short: "Where a card is in its learning journey.",
    body: [
      "New: not yet seen. Learning: seen a few times, still short intervals. Review: learned, now spaced out. Relearning: you forgot it and it is back on short intervals.",
      "In a card's details: Reps is how many times you reviewed it, Lapses how often you forgot it, Stability how many days its memory is expected to last, and Difficulty how hard it is for you (higher is harder).",
    ],
  },
  dueOnly: {
    title: "Due only",
    short: "Show just the cards that are due now.",
    body: ["Turn it on to hide cards that are scheduled for later."],
  },
  reviewAll: {
    title: "Review all",
    short: "Review due cards from every workspace in one go.",
    body: [
      "Each workspace is its own subject with its own cards. Review all walks through them one after another, up to 20 cards from each, so you do not have to switch by hand.",
      "Between workspaces you can continue, skip the next one, or stop. You see a summary at the end.",
    ],
  },
} satisfies Record<string, InfoContent>

export const reviewGuide = {
  title: "About Review",
  summary:
    "Review is your daily memory practice. The app quizzes you on cards and schedules each one for just before you would forget it.",
  parts: [
    reviewInfo.review,
    reviewInfo.spaced,
    reviewInfo.due,
    reviewInfo.newToday,
    reviewInfo.reviewed,
    reviewInfo.streak,
    reviewInfo.retention,
    reviewInfo.forecast,
    reviewInfo.controls,
    reviewInfo.card,
    reviewInfo.progress,
    reviewInfo.verdict,
    reviewInfo.rating,
    reviewInfo.summary,
    reviewInfo.cardsBrowser,
    reviewInfo.cardState,
    reviewInfo.dueOnly,
  ] satisfies InfoContent[],
}

export const reviewAllGuide = {
  title: "About Review all",
  summary:
    "Review all runs your review sessions for every workspace, one after another.",
  parts: [
    reviewInfo.reviewAll,
    reviewInfo.spaced,
    reviewInfo.card,
    reviewInfo.rating,
    reviewInfo.summary,
  ] satisfies InfoContent[],
}
