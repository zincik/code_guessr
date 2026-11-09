#Code Guessr

Test knowledge of your codebase, run this and be tasked to find a line in your code.

##How to use

```code_guessr <file_extension> [directory]```

example:

```code_guessr rs```

Set the minimum number of non-whitespace characters required for a line with
`--min-chars` (or `-m`). It defaults to `40`.

```code_guessr rs ./src --min-chars 20```

##Details

- Recursively walks the selected dir and picks lines over 40 non-whitespace chars. It then picks 1 line at random.
- You have 1 chance to submit
- Duplicate lines are allowed (same line content, different source file and linenumber)
- Shows surrounding 2 lines by reading the sourcefile in the file browser
- You are scored based on time taken

Enjoy playing!
