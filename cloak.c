#include "cloak.h"
#include <stdio.h>
#include <windows.h>
#include <unknwn.h>
#include <objbase.h>

HRESULT SetCloakForWindow(HWND hwnd, APPLICATION_VIEW_CLOAK_TYPE cloakType, int cloakFlag)
{
    /*HRESULT hr = CoInitializeEx(NULL, COINIT_APARTMENTTHREADED);*/
    /*if (FAILED(hr))*/
    /*{*/
    /*    //wprintf(L"[!] CoInitializeEx failed: 0x%08X\n", hr);*/
    /*    return hr;*/
    /*}*/

    IServiceProvider* pServiceProvider = NULL;
    HRESULT hr = CoCreateInstance(&CLSID_ImmersiveShell,
                          NULL,
                          CLSCTX_ALL,
                          &IID_IServiceProvider,
                          (void**)&pServiceProvider);
    if (FAILED(hr) || !pServiceProvider)
    {
        //wprintf(L"[!] CoCreateInstance(CLSID_ImmersiveShell) failed: 0x%08X\n", hr);
        CoUninitialize();
        return hr;
    }

    IAppViewCollectionCustom* pCollection = NULL;
    hr = pServiceProvider->lpVtbl->QueryService(pServiceProvider,
                                                &IID_IAppViewCollectionCustom,
                                                &IID_IAppViewCollectionCustom,
                                                (void**)&pCollection);
    if (SUCCEEDED(hr) && pCollection)
    {
        IApplicationView* pView = NULL;
        hr = pCollection->lpVtbl->GetViewForHwnd(pCollection, hwnd, &pView);
        if (SUCCEEDED(hr) && pView)
        {
            hr = pView->lpVtbl->SetCloak(pView, cloakType, cloakFlag);
            if (SUCCEEDED(hr))
            {
                //wprintf(L"[*] pView->SetCloak succeeded.\n");
            }
            else
            {
                //wprintf(L"[!] pView->SetCloak failed: 0x%08X\n", hr);
            }
            pView->lpVtbl->Release(pView);
        }
        else
        {
            //wprintf(L"[!] GetViewForHwnd failed: 0x%08X\n", hr);
        }
        pCollection->lpVtbl->Release(pCollection);
    }
    else
    {
        //wprintf(L"[!] QueryService(IAppViewCollectionCustom) failed: 0x%08X\n", hr);
    }

    pServiceProvider->lpVtbl->Release(pServiceProvider);
    //CoUninitialize();
    return hr;
}
