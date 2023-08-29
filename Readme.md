# yash
Yet Another SHell

## Why?
Just a little experiment to learn how shells work and what takes to make one

## How to make a cool prompt
To make a prompt, you change the variable `$PS1`
| char | meaning |
| --- | --- |
| n | Current user's username | 
| m | Machine's hostname |
| h | Current working directory, replaces `$HOME` with ~ |
| F{#rrggbb} | Set the foreground color to `#rrggbb` |
| f | Reset foreground color  |

Example:
```bash
PS1="%F{#ff8080}%h%f $ "
```

![default_prompt](img/default_prompt.png)
