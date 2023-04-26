# Decky Translate
### An OCR tool for learning Mandarin while playing video games
Example:

https://user-images.githubusercontent.com/11338764/234514029-3f5ae359-fe09-4695-9390-b38ac1cc5e5a.mov

This is an attempt to recreate the wonderful OCR functionality in the Pleco Android app (and probably also in the iOS app) in a way that I can use it on my Steam Deck.

I am using tesseract to perform OCR of the screen, and using the rust Chinese_Dictionary crate to tokenize and provide translations for that text.

Tesseract is unfortunately somewhat lacking. Primarily, the positioning data that it provides for each character is significantly off, so I can't draw the characters directly over the screen like Pleco does, I am instead drawing a white background and drawing my OCR'd characters onto it. Secondarily, the OCR results are admirable but often not perfect, and currently I get great results in optimal scenarios such as in the example video with consistent text in a textbox with a while background, but much deviation from that and it starts to struggle.

The app will remember its size and position, so in a game such as Pokemon where you'll probably always want to scan the same portion of the screen you won't need to drag it into place ever time. Whenever the window is resized or repositioned in any way it will trigger another scan, so if the OCR hasn't quite worked it is sometimes worth moving the screen slightly and trying again.

Currently only Mandarin is supported, you can choose between Traditional and Simplified by adding **language="ChiTra"** or **language="ChiSim"** to the **[other]** section of the config.ini file.

Next step is to determine how to get this onto a steam deck, preferably as a decky plugin as the name implies. That being said it does work on Linux perfectly well, and would probably work on Windows too, so I might make that a stretch goal after getting the decky part working.
