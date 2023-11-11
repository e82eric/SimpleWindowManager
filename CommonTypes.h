#ifndef COMMON_TYPES_H
#define COMMON_TYPES_H

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
    COLORREF extraFocusBackgroundColor;
    HBRUSH _extraFocusBackgroundBrush;
    COLORREF infoColor;
} TextStyle;

#endif // COMMON_TYPES_H
