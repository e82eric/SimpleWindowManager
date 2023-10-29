#include <windows.h>
#ifdef __cplusplus
extern "C" {
#endif
/* HWND dcomp_border_run(HINSTANCE module); */
void dcomp_border_clean();
void dcomp_border_window_init(HWND window, COLORREF normalColor, COLORREF lostFocusColor);
void dcomp_border_window_draw(UINT height, UINT width, BOOL lostFocus);
#ifdef __cplusplus
}
#endif
