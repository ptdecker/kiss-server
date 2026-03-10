# ptodd.org Backend Framework

This is the repository for the front- and back-ends supporting ptodd.org.

It is built "the hard way" intentionally avoiding third-party crates with a
preference to implement all capabilities using only the standard library. The
exception to this is the usage of the [log](https://crates.io/crates/log)
crate as a facade over the included logging implementation.

## Bare EC2 instance setup

```bash
sudo dnf install git-all
sudo dnf install golang
git clone https://github.com/cli/cli.git gh-cli
cd gh-cli
make install
ssh-keygen -t rsa -b 4096 -C "ptdecker@mac.com"
eval "$(ssh-agent -s)"
ssh-add /home/ec2-user/.ssh/id_rsa
cat ~/.ssh/id_rsa.pub
git clone git@github.com:ptdecker/ptodd.git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cargo install just
just build
```

## Error handling pattern

This repository uses the error handling pattern as discussed in [Jeremy Chone](https://jeremychone.com/)'s
"[Rust Error Handling--Best Practices](https://www.youtube.com/watch?v=j-VQCYP7wyw)" YouTube video with the omission
of the usage of the third-party [derive_more](https://jeltef.github.io/derive_more/derive_more/index.html) crate in
keeping with this project's goal of avoiding third-party crates.
(Cf. [Error Handling](https://rust10x.com/best-practices/error-handling))

## Learning Resources

* [How Unique Is Your Web Browser](https://share.evernote.com/note/30e0c841-bf39-4d3e-8eb0-ca325a60732a)
