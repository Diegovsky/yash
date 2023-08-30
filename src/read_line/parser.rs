
#[derive(Logos, Debug, PartialEq)]
 #[logos(skip r"[ \t]+")] // Ignore this regex pattern between tokens
 enum Token {
    #[token("|")]
    Pipe,
    #[token(">")]
    Redir,
    #[regex("\w+")]
    Word,
 }
