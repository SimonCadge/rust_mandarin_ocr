# Rust Mandarin OCR
### An OCR tool for learning Mandarin while playing video games
Example:

https://user-images.githubusercontent.com/11338764/234514029-3f5ae359-fe09-4695-9390-b38ac1cc5e5a.mov

A rust native recreation of the wonderful OCR functionality in the Pleco Android app, for use as a study aid while playing video games in Mandarin.

I am using tesseract to perform OCR of the screen, and using the rust Chinese_Dictionary crate to tokenize and provide translations for that text.

Tesseract is unfortunately somewhat lacking. Primarily, the positioning data that it provides for each character is significantly off, so I can't draw the characters directly over the screen like Pleco does. I am instead drawing a white background and drawing my OCR'd characters onto it. Secondarily, the OCR results are admirable but often not perfect, and currently I get great results in optimal scenarios such as in the example video with consistent text in a textbox with a while background, but much deviation from that and it starts to struggle.

The app will remember the most recent size and position, so in a game such as Pokemon where you'll probably always want to scan the same portion of the screen you won't need to drag it into place ever time. Whenever the window is resized or repositioned in any way it will trigger another scan, so if the OCR hasn't quite worked it is sometimes worth moving the screen slightly and trying again.

Currently only Mandarin is supported, you can choose between Traditional and Simplified by adding **language="ChiTra"** or **language="ChiSim"** to the **[other]** section of the config.ini file.

Initially the plan was to load this as a steam deck plugin using Decky Loader. Having looked at the decky plugin structure it is clear that the plugin would need to be written in python and typescript, which sounds like a fun project, so I for now this project is considered a complete MVP.
