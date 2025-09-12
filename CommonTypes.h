#ifndef COMMON_TYPES_H
#define COMMON_TYPES_H

#include <windows.h>
#include <stdbool.h>

typedef struct TextStyle
{
    HFONT font;
    HFONT iconFont;
    COLORREF textColor;
    COLORREF backgroundColor;
    HBRUSH _backgroundBrush;
    COLORREF disabledColor;
    COLORREF focusBackgroundColor;
    HBRUSH _focusBackgroundBrush;
    COLORREF focusTextColor;
    COLORREF focusColor2;
    COLORREF lostFocusColor;
    HPEN _focusPen2; 
    COLORREF extraFocusBackgroundColor;
    HBRUSH _extraFocusBackgroundBrush;
    COLORREF infoColor;
    int borderWidth;
} TextStyle;

#endif
