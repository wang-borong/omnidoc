%%
%% Copyright © 2023 Jason Wang. All Rights Reserved.
%%
%% This is the latex class for writing documents.
%%
%% This document has two options,
%% 1. fontset = {msword, siyuan} to set font family to Microsoft fonts
%%    or to siyuan family fonts
%% 2. linestretch = {100pt, 5cm} to set line stretch to ctex
%% Except the two options, it also support use the ctex's options.

\NeedsTeXFormat{LaTeX2e}
\ProvidesClass{ctbook}[2023/09/11 v0.3 a book class derived from ctexbook]

\RequirePackage{kvoptions}
\RequirePackage{etoolbox}
\SetupKeyvalOptions{family=ctbook,prefix=ctbook@,setkeys=\kvsetkeys}
%\newcommand{\wkv}[1]{\kvsetkeys{ctbook}{#1}}

\DeclareStringOption[msword]{fontset}
\DeclareStringOption[10]{linestretch}

\DeclareDefaultOption{\PassOptionsToClass{\CurrentOption}{ctexbook}}

\ProcessKeyvalOptions*\relax

\LoadClass[
  UTF8,
  scheme = chinese,
  heading = true,
  sub4section,
  fontset=\ctbook@fontset,
]{ctexbook}

\ctexset{
  % 设置汉字之间弹性间距的弹性程度。如果有单位，则可以在选项中直接写；
  % 如果是数字，单位则是汉字宽度 \ccwd 的倍数。
  linestretch = \ctbook@linestretch,
  % 使用 sub4section 还需要将计数器设置成 5
  secnumdepth = 5,
  % 将目录层级设为 3
  tocdepth = 3,
}

% 用于临时设置行间距
\RequirePackage{setspace}

% 设置字体
\RequirePackage{fontspec}
% \ifdefstring{\ctbook@fontset}{msword}{
%   % 设置西文字体为Arial，其他字体暂时保持不变
%   \setmainfont{Arial}
%   \setmonofont{Source Code Pro}
% }{
%   \setmainfont{Source Serif Pro}
%   \setsansfont{Source Sans Pro}
%   \setmonofont{Source Code Pro}
% }

% 调整页面大小
\RequirePackage{geometry}

% 用于插入数学公式
\RequirePackage{amsmath,amsthm,amsfonts,amssymb,bm}

% 链接点击相关包
\RequirePackage{hyperref}

% 图表相关
\RequirePackage{float,graphicx,tikz,array}
\floatplacement{figure}{H}

\RequirePackage{tabularx}
\RequirePackage{booktabs}
\RequirePackage{colortbl}

\RequirePackage{biblatex}
\nocite{*}

% 批量设置不同的列表编号
\RequirePackage{enumerate}

% 代码高亮相关
\RequirePackage{listings-config}
% Override basicstyle for this specific template
\lstset{basicstyle=\zihao{-5}\ttfamily}

\RequirePackage{helper}
\RequirePackage{graphs}

\endinput
