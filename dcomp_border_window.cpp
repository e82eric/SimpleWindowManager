#ifndef UNICODE
#define UNICODE
#endif

#include <windows.h>
#include <wrl.h>
using namespace Microsoft::WRL;
#include <dxgi1_3.h>
#include <d3d11_2.h>
#include <d2d1_2.h>
#include <d2d1_2helper.h>
#include <dcomp.h>

#include "dcomp_border_window.h"

HWND g_hwnd;

ComPtr<ID3D11Device> g_direct3dDevice;
ComPtr<IDXGIDevice> g_dxgiDevice;
ComPtr<IDXGIFactory2> g_dxFactory;
ComPtr<IDXGISwapChain1> g_swapChain;
ComPtr<ID2D1Factory2> g_d2Factory;
ComPtr<ID2D1Device1> g_d2Device;
ComPtr<ID2D1DeviceContext> g_dc;
ComPtr<IDCompositionDevice> g_dcompDevice;
ComPtr<IDCompositionTarget> g_target;
ComPtr<IDCompositionVisual> g_visual;
ComPtr<IDXGISurface2> g_surface;
ComPtr<ID2D1Bitmap1> g_bitmap;
ComPtr<ID2D1SolidColorBrush> g_normalBrush;
ComPtr<ID2D1SolidColorBrush> g_lostFocusBrush;

struct ComException
{
    HRESULT result;
    ComException(HRESULT const value) :
        result(value)
    {}
};

void HR(HRESULT const result)
{
    if (S_OK != result)
    {
        throw ComException(result);
    }
}

void ResizeSwapChain()
{
    if (g_dc) g_dc->SetTarget(nullptr);
    g_bitmap = nullptr;
    g_surface = nullptr;

    RECT rect;
    GetClientRect(g_hwnd, &rect);
    UINT width = rect.right - rect.left;
    UINT height = rect.bottom - rect.top;

    if (g_swapChain)
    {
        HR(g_swapChain->ResizeBuffers(2, width, height, DXGI_FORMAT_B8G8R8A8_UNORM, 0));
    }

    HR(g_swapChain->GetBuffer(0, __uuidof(g_surface), reinterpret_cast<void**>(g_surface.GetAddressOf())));

    D2D1_BITMAP_PROPERTIES1 properties = {};
    properties.pixelFormat.alphaMode = D2D1_ALPHA_MODE_PREMULTIPLIED;
    properties.pixelFormat.format = DXGI_FORMAT_B8G8R8A8_UNORM;
    properties.bitmapOptions = D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW;

    HR(g_dc->CreateBitmapFromDxgiSurface(g_surface.Get(), properties, g_bitmap.GetAddressOf()));
}

void Draw(ComPtr<ID2D1SolidColorBrush> brush)
{
    RECT rect = {};
    GetClientRect(g_hwnd, &rect);

    PAINTSTRUCT ps;
    BeginPaint(g_hwnd, &ps);
    g_dc->SetTarget(g_bitmap.Get());
    g_dc->BeginDraw();

    D2D1_COLOR_F const brushColor = D2D1::ColorF(0.0f,
            0.0f,
            0.0f,
            0.5f);

    g_dc->Clear(brushColor);
    D2D1_RECT_F rect2 = D2D1::RectF(0.0f, 0.0f, static_cast<float>(rect.right), static_cast<float>(rect.bottom));
    FLOAT borderThickness = 10.0f;
    g_dc->DrawRectangle(&rect2, brush.Get(), borderThickness);

    HR(g_dc->EndDraw());
    HR(g_swapChain->Present(1, 0));

    HR(g_visual->SetContent(g_swapChain.Get()));

    HR(g_target->SetRoot(g_visual.Get()));

    HR(g_dcompDevice->Commit());
    EndPaint(g_hwnd, &ps);
}

void dcomp_border_window_draw(UINT height, UINT width, BOOL lostFocus)
{
    static UINT oldWidth = 0;
    static UINT oldHeight = 0;
    static BOOL oldLostFocus = FALSE;

    if((oldWidth != width || oldHeight != height || oldLostFocus != lostFocus) && (width > 0 && height > 0))
    {
        ComPtr<ID2D1SolidColorBrush> brush = g_normalBrush;
        if(lostFocus)
        {
            brush = g_lostFocusBrush;
        }
        ResizeSwapChain();
        Draw(brush);
        oldWidth = width;
        oldHeight = height;
        oldLostFocus = lostFocus;
    }
}

D2D1_COLOR_F _ColorRefToD2D1ColorF(COLORREF color)
{
    float r = (float)(GetRValue(color)) / 255.0f;
    float g = (float)(GetGValue(color)) / 255.0f;
    float b = (float)(GetBValue(color)) / 255.0f;

    D2D1_COLOR_F d2d1Color = { r, g, b, 1.0f };
    return d2d1Color;
}

void dcomp_border_window_init(HWND window, COLORREF normalColor, COLORREF lostFocusColor)
{
    g_hwnd = window;
    if(!g_direct3dDevice)
    {
        HR(D3D11CreateDevice(nullptr,
                    D3D_DRIVER_TYPE_HARDWARE,
                    nullptr,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    nullptr, 0,
                    D3D11_SDK_VERSION,
                    &g_direct3dDevice,
                    nullptr,
                    nullptr));

        HR(g_direct3dDevice.As(&g_dxgiDevice));

        HR(CreateDXGIFactory2(0, __uuidof(g_dxFactory), reinterpret_cast<void**>(g_dxFactory.GetAddressOf())));

        DXGI_SWAP_CHAIN_DESC1 description = {};
        description.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
        description.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
        description.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;
        description.BufferCount = 2;
        description.SampleDesc.Count = 1;
        description.AlphaMode = DXGI_ALPHA_MODE_PREMULTIPLIED;

        RECT rect = {};
        GetClientRect(g_hwnd, &rect);
        description.Width = rect.right - rect.left;
        description.Height = rect.bottom - rect.top;

        HR(g_dxFactory->CreateSwapChainForComposition(g_dxgiDevice.Get(),
                    &description,
                    nullptr,
                    g_swapChain.GetAddressOf()));

        D2D1_FACTORY_OPTIONS const options = { 0 };
        HR(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED,
                    options,
                    g_d2Factory.GetAddressOf()));

        HR(g_d2Factory->CreateDevice(g_dxgiDevice.Get(),
                    g_d2Device.GetAddressOf()));

        HR(g_d2Device->CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
                    g_dc.GetAddressOf()));

        HR(g_swapChain->GetBuffer(
                    0,
                    __uuidof(g_surface),
                    reinterpret_cast<void**>(g_surface.GetAddressOf())));

        D2D1_BITMAP_PROPERTIES1 properties = {};
        properties.pixelFormat.alphaMode = D2D1_ALPHA_MODE_PREMULTIPLIED;
        properties.pixelFormat.format = DXGI_FORMAT_B8G8R8A8_UNORM;
        properties.bitmapOptions = D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW;

        HR(g_dc->CreateBitmapFromDxgiSurface(g_surface.Get(),
                    properties,
                    g_bitmap.GetAddressOf()));

        D2D1_COLOR_F normalD2DColor = _ColorRefToD2D1ColorF(normalColor);
        D2D1_COLOR_F lostFocusD2DColor = _ColorRefToD2D1ColorF(lostFocusColor);

        HR(g_dc->CreateSolidColorBrush(normalD2DColor,
                    g_normalBrush.GetAddressOf()));

        HR(g_dc->CreateSolidColorBrush(lostFocusD2DColor,
                    g_lostFocusBrush.GetAddressOf()));

        HR(DCompositionCreateDevice(
                    g_dxgiDevice.Get(),
                    __uuidof(g_dcompDevice),
                    reinterpret_cast<void**>(g_dcompDevice.GetAddressOf())));

        HR(g_dcompDevice->CreateTargetForHwnd(g_hwnd,
                    true,
                    g_target.GetAddressOf()));

        HR(g_dcompDevice->CreateVisual(g_visual.GetAddressOf()));
    }
}

void dcomp_border_clean()
{
    g_normalBrush = nullptr;
    g_bitmap = nullptr;
    g_surface = nullptr;
    g_visual = nullptr;
    g_target = nullptr;
    g_dcompDevice = nullptr;
    g_dc = nullptr;
    g_d2Device = nullptr;
    g_d2Factory = nullptr;
    g_swapChain = nullptr;
    g_dxFactory = nullptr;
    g_dxgiDevice = nullptr;
    g_direct3dDevice = nullptr;
}
