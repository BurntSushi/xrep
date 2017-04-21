Name:           ripgrep
Version:        0.3.2
Release:        1%{?dist}
Summary:        A search tool that combines the usability of ag with the raw speed of grep

License:        MIT or UNLICENSE
URL:            https://github.com/BurntSushi/%{name}
Source0:        https://github.com/BurntSushi/%{name}/archive/%{version}/%{name}-%{version}.tar.gz
Source1:        rg.bash-completion
Source2:        rg.fish
Source3:        _rg

BuildRequires:  cargo

%description
%{name} is a line oriented search tool that combines the usability of The
Silver Searcher (an ack clone) with the raw speed of GNU grep. %{name} works
by recursively searching your current directory for a regex pattern.

%package bash-completion
Summary:        bash completion files for %{name}
Requires:       %{name} = %{version}-%{release}
Requires:       bash-completion

%description bash-completion
This package contains the bash completion files for %{name}.

%package fish-completion
Summary:        fish completion files for %{name}
Requires:       %{name} = %{version}-%{release}
Requires:       fish

%description fish-completion
This package contains the fish completion files for %{name}.

%package zsh-completion
Summary:        zsh completion files for %{name}
Requires:       %{name} = %{version}-%{release}
Requires:       zsh

%description zsh-completion
This package contains the zsh completion files for %{name}.

%prep
%autosetup

%build
cargo build --release

%install
install -D -p -m 755 target/release/rg $RPM_BUILD_ROOT%{_bindir}/rg
install -D -p -m 644 doc/rg.1 $RPM_BUILD_ROOT%{_mandir}/man1/rg.1

# bash completion
install -D -p -m 644 %SOURCE1 \
    $RPM_BUILD_ROOT%{_datadir}/bash-completion/completions/rg

# fish completion
install -D -p -m 644 %SOURCE2 \
    $RPM_BUILD_ROOT%{_datadir}/fish/vendor_completions.d/rg.fish

# zsh completion
install -D -p -m 644 %SOURCE3 \
    $RPM_BUILD_ROOT%{_datadir}/zsh/site-functions/_rg

%check
cargo test

%files
%license COPYING LICENSE-MIT UNLICENSE
%doc CHANGELOG.md README.md
%{_bindir}/rg
%{_mandir}/man1/rg.1*

%files bash-completion
%{_datadir}/bash-completion/completions/rg

%files fish-completion
%{_datadir}/fish/vendor_completions.d/rg.fish

%files zsh-completion
%{_datadir}/zsh/site-functions/_rg

%changelog
* Sat Dec 24 2016 Filip Szymański <fszymanski at, fedoraproject.org> - 0.3.2-1
- Initial release
