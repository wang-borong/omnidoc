%%
%% Copyright © 2023 Jason Wang. All Rights Reserved.
%%
%% This is the latex class for writing documents.
%%
%% This document has two options,
%% 1. fontset = {msword, siyuan} to set font family to Microsoft fonts
%%    or to siyuan family fonts
%% 2. linestretch = {100pt, 5cm} to set line stretch to ctexart
%% Except the two options, it also support use the ctexart's options.

\NeedsTeXFormat{LaTeX2e}
\ProvidesClass{ctart}[2023/09/11 v0.3 an article class derived from ctexart]

\RequirePackage{kvoptions}
\RequirePackage{etoolbox}
\SetupKeyvalOptions{family=ctart,prefix=ctart@,setkeys=\kvsetkeys}
%\newcommand{\wkv}[1]{\kvsetkeys{ctart}{#1}}

\DeclareStringOption[msword]{fontset}
\DeclareStringOption[10]{linestretch}

\DeclareDefaultOption{\PassOptionsToClass{\CurrentOption}{ctexart}}

\ProcessKeyvalOptions*\relax

% scheme = chinese: 对 beamer 以外的文档类，调整默认字号为五号字，
%                   并调整行距为 1.3；汉化文档中的标题名字；
%                   在 heading = true 的情况下还会将章节标题的风格修改为中文样式。
%          plain:   不调整默认字号和行距，不会汉化文档中的标题名字，
%                   也不会将章节标题风格修改为中文样式，
%                   同时不会调整 \pagestyle，并禁用 autoindent 选项。
%                   事实上，此时的 CTEX 宏集只提供了中
%                   文支持功能，而不对文章版式进行任何修改。
% sub4section: 使用 5 级标题
% linespread=1.5: 设置行距倍数为 1.5 倍
\LoadClass[
  UTF8,
  a4paper,
  scheme = chinese,
  heading = true,
  zihao = 5,
  sub4section,
  linespread = 1.5,
  fontset=\ctart@fontset,
]{ctexart}

\ctexset{
  % 设置汉字之间弹性间距的弹性程度。如果有单位，则可以在选项中直接写；
  % 如果是数字，单位则是汉字宽度 \ccwd 的倍数。
  linestretch = \ctart@linestretch,
  % 使用 sub4section 还需要将计数器设置成 5
  secnumdepth = 5,
  % 将目录层级设为 3
  tocdepth = 3,
}

% 用于临时设置行间距
\RequirePackage{setspace}

% 设置字体
\RequirePackage{fontspec}
% \setmainfont{Arial}
\setmonofont{Source Code Pro}

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

% 批量设置不同的列表编号
\RequirePackage{enumerate}

\RequirePackage{biblatex}
\nocite{*}

% 代码高亮相关
\RequirePackage{listings-config}
% Override basicstyle for this specific template
\lstset{basicstyle=\zihao{-5}\ttfamily}

\RequirePackage{helper}
\RequirePackage{graphs}

\endinput
