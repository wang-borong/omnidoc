#!/usr/bin/env perl -w

# Ignore any locally installed files to make builds reproducible
#
# (? is a deliberately chosen, invalid path. Unsetting the environment
# variable or setting it to the empty string would have LaTeX search the
# default texmf directory location, which we can only avoid by using an
# invalid path)
if ((not defined $ENV{"TEXMFHOME"}) or ($ENV{"TEXMFHOME"} eq "")) {
    ensure_path('TEXMFHOME', "$ENV{HOME}/.local/share/omnidoc/texmf");
} else {
    ensure_path('TEXMFHOME', "$ENV{HOME}/.local/share/omnidoc/texmf", $ENV{TEXMFHOME});
}
# PDF-generating modes are:
# 1: pdflatex, as specified by $pdflatex variable (still largely in use)
# 2: postscript conversion, as specified by the $ps2pdf variable (useless)
# 3: dvi conversion, as specified by the $dvipdf variable (useless)
# 4: lualatex, as specified by the $lualatex variable (best)
# 5: xelatex, as specified by the $xelatex variable (second best)
$pdf_mode = 5;
# xelatex
$postscript_mode = $dvi_mode = 0;

# Treat undefined references and citations as well as multiply defined references as
# ERRORS instead of WARNINGS.
# This is only checked in the *last* run, since naturally, there are undefined references
# in initial runs.
# This setting is potentially annoying when debugging/editing, but highly desirable
# in the CI pipeline, where such a warning should result in a failed pipeline, since the
# final document is incomplete/corrupted.
#
# However, I could not eradicate all warnings, so that `latexmk` currently fails with
# this option enabled.
# Specifically, `microtype` fails together with `fontawesome`/`fontawesome5`, see:
# https://tex.stackexchange.com/a/547514/120853
# The fix in that answer did not help.
# Setting `verbose=silent` to mute `microtype` warnings did not work.
# Switching between `fontawesome` and `fontawesome5` did not help.
$warnings_as_errors = 0;

# Show used CPU time. Looks like: https://tex.stackexchange.com/a/312224/120853
$show_time = 1;

# option 2 is same as 1 (run biber when necessary), but also deletes the
# regeneratable bbl-file in a clenaup (`latexmk -c`). Do not use if original
# bib file is not available!
$bibtex_use = 1;  # default: 1

# Change default `biber` call, help catch errors faster/clearer. See
# https://web.archive.org/web/20200526101657/https://www.semipol.de/2018/06/12/latex-best-practices.html#database-entries
$biber = "biber --validate-datamodel %O %S";

$interaction = "nonstopmode";

# Reset all search paths
if ((not defined $ENV{"BIBINPUTS"}) or ($ENV{"BIBINPUTS"} eq "")) {
    ensure_path('BIBINPUTS', "biblio")
} else {
    ensure_path('BIBINPUTS', "biblio", "$ENV{BIBINPUTS}")
}
# $ENV{"BSTINPUTS"} = "./include//:";
if ((not defined $ENV{"TEXINPUTS"}) or ($ENV{"TEXINPUTS"} eq "")) {
    ensure_path('TEXINPUTS', "tex");
} else {
    ensure_path('TEXINPUTS', "tex", "$ENV{TEXINPUTS}");
}

$clean_ext = 'synctex.gz';

# Make it compatible with sphinx
if (defined $ENV{OUTDIR}) {
	$out_dir = $ENV{OUTDIR};
}

$pdf_update_method = 2;
# $pdf_previewer = "xdg-open %S";
$pdf_previewer = "zathura %O %S";

#
# To enable shell-escape for all *latex commands  
#   Used i.e. for svg package invoking inkscape
#
set_tex_cmds( '-halt-on-error --shell-escape -file-line-error %O %S' );

# Grabbed from latexmk CTAN distribution:
# Implementing glossary with bib2gls and glossaries-extra, with the
# log file (.glg) analyzed to get dependence on a .bib file.
# !!! ONLY WORKS WITH VERSION 4.54 or higher of latexmk

# Add custom dependency.
# latexmk checks whether a file with ending as given in the 2nd
# argument exists ('toextension'). If yes, check if file with
# ending as in first argument ('fromextension') exists. If yes,
# run subroutine as given in fourth argument.
# Third argument is whether file MUST exist. If 0, no action taken.
add_cus_dep('aux', 'glstex', 0, 'run_bib2gls');

# PERL subroutine. $_[0] is the argument (filename in this case).
# File from author from here: https://tex.stackexchange.com/a/401979/120853
sub run_bib2gls {
  if ( $silent ) {
    # my $ret = system "bib2gls --silent --group '$_[0]'"; # Original version
    my $ret = system "bib2gls --silent --group $_[0]"; # Runs in PowerShell
  } else {
    # my $ret = system "bib2gls --group '$_[0]'"; # Original version
    my $ret = system "bib2gls --group $_[0]"; # Runs in PowerShell
  };
  
  my ($base, $path) = fileparse( $_[0] );
  if ($path && -e "$base.glstex") {
    rename "$base.glstex", "$path$base.glstex";
  }
  
  # Analyze log file.
  local *LOG;
  $LOG = "$_[0].glg";
  if (!$ret && -e $LOG) {
    open LOG, "<$LOG";
    while (<LOG>) {
      if (/^Reading (.*\.bib)\s$/) {
        rdb_ensure_file( $rule, $1 );
      }
    }
    close LOG;
  }
  return $ret;
}

