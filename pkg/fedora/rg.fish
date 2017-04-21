function __fish_using_command
    set cmd (commandline -opc)
    if [ (count $cmd) -eq (count $argv) ]
        for i in (seq (count $argv))
            if [ $cmd[$i] != $argv[$i] ]
                return 1
            end
        end
        return 0
    end
    return 1
end

complete -c rg -n "__fish_using_command rg" -s e -l regexp -d "A regular expression used for searching."
complete -c rg -n "__fish_using_command rg" -l color -d "When to use color. [default: auto]" -r -f -a "never auto always ansi"
complete -c rg -n "__fish_using_command rg" -l colors -d "Configure color settings and styles."
complete -c rg -n "__fish_using_command rg" -s g -l glob -d "Include or exclude files/directories."
complete -c rg -n "__fish_using_command rg" -s t -l type -d "Only search files matching TYPE."
complete -c rg -n "__fish_using_command rg" -s T -l type-not -d "Do not search files matching TYPE."
complete -c rg -n "__fish_using_command rg" -s A -l after-context -d "Show NUM lines after each match."
complete -c rg -n "__fish_using_command rg" -s B -l before-context -d "Show NUM lines before each match."
complete -c rg -n "__fish_using_command rg" -s C -l context -d "Show NUM lines before and after each match."
complete -c rg -n "__fish_using_command rg" -l context-separator -d "Set the context separator string. [default: --]"
complete -c rg -n "__fish_using_command rg" -s f -l file -d "Search for patterns from the given file."
complete -c rg -n "__fish_using_command rg" -l ignore-file -d "Specify additional ignore files."
complete -c rg -n "__fish_using_command rg" -s m -l max-count -d "Limit the number of matches."
complete -c rg -n "__fish_using_command rg" -l maxdepth -d "Descend at most NUM directories."
complete -c rg -n "__fish_using_command rg" -s r -l replace -d "Replace matches with string given."
complete -c rg -n "__fish_using_command rg" -s j -l threads -d "The approximate number of threads to use."
complete -c rg -n "__fish_using_command rg" -l type-add -d "Add a new glob for a file type."
complete -c rg -n "__fish_using_command rg" -l type-clear -d "Clear globs for given file type."
complete -c rg -n "__fish_using_command rg" -s h -d "Show short help output."
complete -c rg -n "__fish_using_command rg" -l help -d "Show verbose help output."
complete -c rg -n "__fish_using_command rg" -s V -l version -d "Prints version information."
complete -c rg -n "__fish_using_command rg" -l files -d "Print each file that would be searched."
complete -c rg -n "__fish_using_command rg" -l type-list -d "Show all supported file types."
complete -c rg -n "__fish_using_command rg" -s a -l text -d "Search binary files as if they were text."
complete -c rg -n "__fish_using_command rg" -s c -l count -d "Only show count of matches for each file."
complete -c rg -n "__fish_using_command rg" -s F -l fixed-strings -d "Treat the pattern as a literal string."
complete -c rg -n "__fish_using_command rg" -s i -l ignore-case -d "Case insensitive search."
complete -c rg -n "__fish_using_command rg" -s n -l line-number -d "Show line numbers."
complete -c rg -n "__fish_using_command rg" -s N -l no-line-number -d "Suppress line numbers."
complete -c rg -n "__fish_using_command rg" -s q -l quiet -d "Do not print anything to stdout."
complete -c rg -n "__fish_using_command rg" -s u -l unrestricted -d "Reduce the level of "smart" searching."
complete -c rg -n "__fish_using_command rg" -s v -l invert-match -d "Invert matching."
complete -c rg -n "__fish_using_command rg" -s w -l word-regexp -d "Only show matches surrounded by word boundaries."
complete -c rg -n "__fish_using_command rg" -l column -d "Show column numbers"
complete -c rg -n "__fish_using_command rg" -l debug -d "Show debug messages."
complete -c rg -n "__fish_using_command rg" -s l -l files-with-matches -d "Only show the path of each file with at least one match."
complete -c rg -n "__fish_using_command rg" -l files-without-match -d "Only show the path of each file that contains zero matches."
complete -c rg -n "__fish_using_command rg" -s H -l with-filename -d "Show file name for each match."
complete -c rg -n "__fish_using_command rg" -l no-filename -d "Never show the file name for a match."
complete -c rg -n "__fish_using_command rg" -l heading -d "Show matches grouped by each file."
complete -c rg -n "__fish_using_command rg" -l no-heading -d "Don't group matches by each file."
complete -c rg -n "__fish_using_command rg" -l hidden -d "Search hidden files and directories."
complete -c rg -n "__fish_using_command rg" -s L -l follow -d "Follow symbolic links."
complete -c rg -n "__fish_using_command rg" -l mmap -d "Searching using memory maps when possible."
complete -c rg -n "__fish_using_command rg" -l no-messages -d "Suppress all error messages."
complete -c rg -n "__fish_using_command rg" -l no-mmap -d "Never use memory maps."
complete -c rg -n "__fish_using_command rg" -l no-ignore -d "Don't respect ignore files."
complete -c rg -n "__fish_using_command rg" -l no-ignore-parent -d "Don't respect ignore files in parent directories."
complete -c rg -n "__fish_using_command rg" -l no-ignore-vcs -d "Don't respect VCS ignore files"
complete -c rg -n "__fish_using_command rg" -l null -d "Print NUL byte after file names"
complete -c rg -n "__fish_using_command rg" -s p -l pretty -d "Alias for --color always --heading -n."
complete -c rg -n "__fish_using_command rg" -s s -l case-sensitive -d "Search case sensitively."
complete -c rg -n "__fish_using_command rg" -s S -l smart-case -d "Smart case search."
complete -c rg -n "__fish_using_command rg" -l vimgrep -d "Show results in vim compatible format."
