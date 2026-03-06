FROM archlinux:latest

RUN pacman -Syyuu gcc git python llvm clang rustup npm pre-commit pkgconf --noconfirm --needed && \
    rustup default stable && \
    rustup update && \
    touch /root/.bashrc && \
    cargo install just && \
    echo "export PATH=\"$PATH:$HOME/.cargo/bin\"" | tee -a "$HOME/.bashrc"

ENV PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN git clone https://gitlab.com/byacrates/hoy.git && \
    cd hoy && \
    just init && \
    cd .. && \
    rm -rf hoy

CMD ["bash"]
