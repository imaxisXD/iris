# Iris usability test plan (round 1, simulated)

## What this round is for

Find the obvious problems before real people try Iris: places where someone gets stuck, misreads what Iris did, or loses trust in it. A simulated tester (an AI agent driving the real app with computer use) runs this round. It doesn't replace sessions with real users. The research on LLM-simulated users is consistent here: they're useful for early, systematic checks, they can report problems that aren't real, and they're weak on emotional nuance. So every finding must cite what was on screen (a screenshot), and round 2 must be 5 real people.

## Method

- **Task-based, think-aloud.** The tester works through realistic goals and narrates what it sees, expects, and tries, the same way a person thinks aloud.
- **Goals, not steps.** Tasks say what the person wants to get done, never which button, menu, or shortcut to use, and never use Iris's own words ("panel", "Fix", "tone", "Quick Fix", "Settings"). This follows NN/g's rules for task scenarios: realistic, actionable, no clues.
- **Screen only.** The tester learns Iris only from what Iris shows: onboarding, menus, panels, settings. It does not read the source code, README, or this plan's success criteria.
- **One task at a time, in order.** Later tasks build on earlier ones, like a real first day.
- **After each task:** an ease rating from the persona's point of view (Single Ease Question, 1 = very difficult, 7 = very easy).
- **After all tasks:** a short heuristic pass (Nielsen's 10 heuristics), clearly labeled as an expert review, separate from the persona's experience.

## Persona

A customer-support lead at an online store. English is their second language. They write 40–60 Slack messages a day to teammates and customers. Today they paste drafts into ChatGPT to fix them, which takes time and feels risky with customer details. They're comfortable with a Mac but don't use keyboard shortcuts beyond copy and paste. They worry about sounding rude or awkward, and they're usually in a hurry. They want Iris to fix mistakes without making them sound like someone else.

Behavior to keep in character:
- They skim; they don't read long text unless stuck.
- They click what looks obvious before searching menus.
- They don't know macOS terms like "Accessibility" or "menu bar extra".
- If something is unclear, they say so in their own words.

## Environment and safety rules

- **Practice surfaces only:** `docs/user-testing/practice-chat.html` opened in a browser (a fake chat box), and a new, unsaved TextEdit document.
- **Never** open or type in Slack, Mail, Messages, Teams, Outlook, or anything else that can send a message.
- **Never** change macOS system settings. The owner has already turned on Iris's Accessibility permission. Viewing System Settings is fine.
- **Never** click "Delete" for a downloaded model, quit other apps, or touch files outside the output folder.
- Iris's global shortcuts are system-wide key presses. Use foreground key delivery if background delivery doesn't reach Iris.

## Tasks

| # | Scenario given to the tester | Success (hidden from tester) |
|---|---|---|
| 1 | You just installed Iris after a teammate recommended it. Get it ready so you can use it on your messages. | Model shows as ready, and the tester can say how to start Iris from any app. |
| 2 | In the practice chat, you've typed this reply to your manager: `hey, i have checked the PR and its look fine but one thing need to change before merge @lead`. Make sure it reads correctly before you send it. Then press the practice Send button. | The sent message is the corrected text, and `@lead` is unchanged. |
| 3 | Your next message goes to a sales manager you've never worked with: `can you tell me when the customer order will ship, they are asking since 2 days`. You want it to sound polite and professional. Put that version in the chat box. | The box holds a professional rewrite that keeps the meaning. |
| 4 | On second thought, you liked your own wording better. Get your original message back in the box. | The original text is back (by undo or any other way). Note how they tried. |
| 5 | Check this message before sending: `Can you review this when you get a chance?` | The tester understands nothing needed fixing, and the text is unchanged. |
| 6 | The key combination for Iris is awkward for your hands. Change it to one you find easier, and check that the new one works. | A new shortcut is saved and opens Iris in the practice chat. |
| 7 | Your team writes "Northwind" and "OKR" exactly like that, and Iris must never change them. You also never want jokes in work messages, so get that option out of your way. | Both words are on the never-change list, and Funny is hidden. |
| 8 | You've started trusting Iris and fix a lot of messages a day. Find a faster way to fix a message when you don't need to review every change. Try it on this TextEdit text: `the order got delay because warehouse was closed on friday`. | The tester finds and uses the one-step fix, and the TextEdit text is corrected. |
| 9 | You never want Iris to read anything in TextEdit, because you keep private notes there. Set that up and check it. | TextEdit is on the off list, and pressing the shortcut there shows Iris is off. |
| 10 | Your laptop feels slow and you suspect Iris is using a lot of memory. Make it use less without uninstalling it. | Memory freed, or the smaller model chosen. Nothing deleted. |

## What to record per task

- Think-aloud notes: what the tester saw, expected, and did, with wrong turns.
- Outcome: Pass, Partial, or Fail, judged afterwards against the hidden criteria.
- Steps and rough time.
- Errors: wrong clicks, dead ends, misunderstandings.
- Ease rating, 1–7, in the persona's voice, with one sentence explaining why.
- Screenshots of the key moments, especially every problem.

## Severity scale for problems (Nielsen)

- 0: Not a usability problem.
- 1: Cosmetic. Fix if there's time.
- 2: Minor. Low priority.
- 3: Major. Important to fix.
- 4: Catastrophe. Must fix before anyone else uses it.

## Report format

1. Summary table: task, outcome, ease rating, top problem.
2. Problems, highest severity first. Each one gets the task, what happened, the evidence (screenshot path), why it matters to this persona, and a suggested fix.
3. What worked well.
4. Heuristic review (expert view, separate from the persona).
5. Caveats: where the simulation may be wrong, and what only real users can answer.
