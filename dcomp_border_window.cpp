#ifndef UNICODE
#define UNICODE
#endif
#include <windows.h>
#pragma comment(lib, "user32.lib")

#include <wrl.h>
using namespace Microsoft::WRL;
#include <dxgi1_3.h>
#include <d3d11_2.h>
#include <d2d1_2.h>
#include <d2d1_2helper.h>
#include <dcomp.h>
#pragma comment(lib, "dxgi")
#pragma comment(lib, "d3d11")
#pragma comment(lib, "d2d1")
#pragma comment(lib, "dcomp")

#include "dcomp_border_window.h"

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
ComPtr<ID2D1SolidColorBrush> g_brush;
ComPtr<ID2D1SolidColorBrush> g_brush2;

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

void ResizeSwapChain(HWND window)
{
    if (g_dc) g_dc->SetTarget(nullptr);
    g_bitmap = nullptr;
    g_surface = nullptr;

    RECT rect;
    GetClientRect(window, &rect);
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

void Draw(HWND window)
{
    RECT rect = {};
    GetClientRect(window, &rect);

    PAINTSTRUCT ps;
    BeginPaint(window, &ps);
    g_dc->SetTarget(g_bitmap.Get());
    g_dc->BeginDraw();
    g_dc->Clear();

    D2D1_POINT_2F const ellipseCenter = D2D1::Point2F(150.0f,  // x
        150.0f); // y
    D2D1_ELLIPSE const ellipse = D2D1::Ellipse(ellipseCenter,
        100.0f,  // x radius
        100.0f); // y radius
    g_dc->FillEllipse(ellipse,
        g_brush.Get());

    D2D1_RECT_F rect2 = D2D1::RectF(0.0f, 0.0f, static_cast<float>(rect.right), static_cast<float>(rect.bottom));
    g_dc->FillRectangle(&rect2, g_brush.Get());
    FLOAT borderThickness = 10.0f;
    g_dc->DrawRectangle(&rect2, g_brush2.Get(), borderThickness);

    HR(g_dc->EndDraw());
    HR(g_swapChain->Present(1, 0));

    HR(g_visual->SetContent(g_swapChain.Get()));

    HR(g_target->SetRoot(g_visual.Get()));

    HR(g_dcompDevice->Commit());
    EndPaint(window, &ps);
}

void CreateStuff(HWND window)
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

    HR(CreateDXGIFactory2(
        DXGI_CREATE_FACTORY_DEBUG,
        __uuidof(g_dxFactory),
        reinterpret_cast<void**>(g_dxFactory.GetAddressOf())));

    DXGI_SWAP_CHAIN_DESC1 description = {};

    description.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    description.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    description.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;
    description.BufferCount = 2;
    description.SampleDesc.Count = 1;
    description.AlphaMode = DXGI_ALPHA_MODE_PREMULTIPLIED;

    RECT rect = {};
    GetClientRect(window, &rect);
    description.Width = rect.right - rect.left;
    description.Height = rect.bottom - rect.top;

    HR(g_dxFactory->CreateSwapChainForComposition(g_dxgiDevice.Get(),
        &description,
        nullptr,
        g_swapChain.GetAddressOf()));

    D2D1_FACTORY_OPTIONS const options = { D2D1_DEBUG_LEVEL_INFORMATION };
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
    properties.bitmapOptions = D2D1_BITMAP_OPTIONS_TARGET |
        D2D1_BITMAP_OPTIONS_CANNOT_DRAW;

    HR(g_dc->CreateBitmapFromDxgiSurface(g_surface.Get(),
        properties,
        g_bitmap.GetAddressOf()));
    D2D1_COLOR_F const brushColor = D2D1::ColorF(0.0f,
        0.0f,
        0.0f,
        0.5f);
    D2D1_COLOR_F const brushColor2 = D2D1::ColorF(
            0.980f,
            0.7412f,
            0.1843f,
            1.0f);
    HR(g_dc->CreateSolidColorBrush(brushColor,
        g_brush.GetAddressOf()));
    HR(g_dc->CreateSolidColorBrush(brushColor2,
        g_brush2.GetAddressOf()));

    HR(DCompositionCreateDevice(
        g_dxgiDevice.Get(),
        __uuidof(g_dcompDevice),
        reinterpret_cast<void**>(g_dcompDevice.GetAddressOf())));

    HR(g_dcompDevice->CreateTargetForHwnd(window,
        true, // Top most
        g_target.GetAddressOf()));

    HR(g_dcompDevice->CreateVisual(g_visual.GetAddressOf()));
}

HWND dcomp_border_run(HINSTANCE module)
{
    SetProcessDPIAware();

    WNDCLASS wc = {};
    wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
    wc.hInstance = module;
    wc.lpszClassName = L"window";
    wc.style = CS_HREDRAW | CS_VREDRAW;
    wc.lpfnWndProc =
        [](HWND window, UINT message, WPARAM wparam,
            LPARAM lparam) -> LRESULT
        {
            switch (message)
            {
            case WM_CREATE:
                CreateStuff(window);
                break;
        case WM_WINDOWPOSCHANGING:
            {
                WINDOWPOS* windowPos = (WINDOWPOS*)lparam;
                windowPos->hwndInsertAfter = HWND_BOTTOM;
                return 1;
            }
            case WM_SIZE:
            {
                UINT width = LOWORD(lparam);
                UINT height = HIWORD(lparam);
                D2D1_SIZE_U size;
                size.width = width;
                size.height = height;

                ResizeSwapChain(window);
                Draw(window);
            }
            break;

            case WM_PAINT:
            case WM_DISPLAYCHANGE:
            {
                Draw(window);
            }
            break;

            case WM_ERASEBKGND:
            return 1;

            case WM_DESTROY:
            {
                PostQuitMessage(0);
            }
            break;
            }

            return DefWindowProc(window, message, wparam, lparam);
        };
    RegisterClass(&wc);
    HWND const window = CreateWindowEx(WS_EX_NOREDIRECTIONBITMAP,
        wc.lpszClassName, L"Sample",
        WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
        CW_USEDEFAULT, CW_USEDEFAULT,
        CW_USEDEFAULT, CW_USEDEFAULT,
        nullptr, nullptr, module, nullptr);
    SetWindowLong(window, GWL_STYLE, 0);
    
    return window;
}
