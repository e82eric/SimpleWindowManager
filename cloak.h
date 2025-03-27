#ifndef UNDOCUMENTEDSHELLINTERFACES_H
#define UNDOCUMENTEDSHELLINTERFACES_H

#include <windows.h>
#include <unknwn.h>
#include <objbase.h>
#include <winnt.h>

typedef void* HSTRING;

typedef enum TrustLevel
{
    BaseTrust,
    PartialTrust,
    FullTrust
} TrustLevel;

typedef struct IAsyncCallback IAsyncCallback;
typedef struct IImmersiveMonitor IImmersiveMonitor;
typedef struct IApplicationViewPosition IApplicationViewPosition;
typedef struct IApplicationViewOperation IApplicationViewOperation;

typedef enum APPLICATION_VIEW_CLOAK_TYPE
{
    AVCT_UNKNOWN0 = 0,
    AVCT_UNKNOWN1 = 1,
    AVCT_UNKNOWN2 = 2
} APPLICATION_VIEW_CLOAK_TYPE;

typedef enum APPLICATION_VIEW_COMPATIBILITY_POLICY
{
    AVCP_NONE,
    AVCP_SMALL,
    AVCP_MEDIUM,
    AVCP_LARGE
} APPLICATION_VIEW_COMPATIBILITY_POLICY;

typedef struct IObjectArray IObjectArray;

#define IImmersiveApplication UINT
#define IApplicationViewChangeListener UINT

#ifndef __IInspectable_INTERFACE_DEFINED__
#define __IInspectable_INTERFACE_DEFINED__

static const IID IID_IInspectable = 
{ 0xaf86e2e0, 0xb12d, 0x4c6a, { 0x9c, 0x5a, 0xd7, 0xaa, 0x65, 0x10, 0x1e, 0x90 } };

typedef struct IInspectable IInspectable;

typedef struct IInspectableVtbl {
    
    HRESULT (*QueryInterface)(IInspectable *This, REFIID riid, void **ppvObject);
    ULONG   (*AddRef)(IInspectable *This);
    ULONG   (*Release)(IInspectable *This);
    
    HRESULT (*GetIids)(IInspectable *This, ULONG *iidCount, IID **iids);
    HRESULT (*GetRuntimeClassName)(IInspectable *This, HSTRING *className);
    HRESULT (*GetTrustLevel)(IInspectable *This, TrustLevel *trustLevel);
} IInspectableVtbl;

struct IInspectable {
    const IInspectableVtbl *lpVtbl;
};

#endif 

#ifndef __IAPPLICATIONVIEW_INTERFACE_DEFINED__
#define __IAPPLICATIONVIEW_INTERFACE_DEFINED__

static const IID IID_IApplicationView = 
{ 0x372e1d3b, 0x38d3, 0x42e4, { 0xa1, 0x5b, 0x8a, 0xb2, 0xb1, 0x78, 0xf5, 0x13 } };

typedef struct IApplicationView IApplicationView;

typedef struct IApplicationViewVtbl {
    HRESULT (*QueryInterface)(IApplicationView *This, REFIID riid, void **ppvObject);
    ULONG   (*AddRef)(IApplicationView *This);
    ULONG   (*Release)(IApplicationView *This);
    
    HRESULT (*GetIids)(IApplicationView *This, ULONG *iidCount, IID **iids);
    HRESULT (*GetRuntimeClassName)(IApplicationView *This, HSTRING *className);
    HRESULT (*GetTrustLevel)(IApplicationView *This, TrustLevel *trustLevel);
    
    HRESULT (*SetFocus)(IApplicationView *This);
    HRESULT (*SwitchTo)(IApplicationView *This);
    HRESULT (*TryInvokeBack)(IApplicationView *This, IAsyncCallback* callback);
    HRESULT (*GetThumbnailWindow)(IApplicationView *This, HWND* phwnd);
    HRESULT (*GetMonitor)(IApplicationView *This, IImmersiveMonitor** monitor);
    HRESULT (*GetVisibility)(IApplicationView *This, int* pVisible);
    HRESULT (*SetCloak)(IApplicationView *This, APPLICATION_VIEW_CLOAK_TYPE cloakType, int unknown);
    HRESULT (*GetPosition)(IApplicationView *This, REFIID, void** outPosition);
    HRESULT (*SetPosition)(IApplicationView *This, IApplicationViewPosition* pos);
    HRESULT (*InsertAfterWindow)(IApplicationView *This, HWND);
    HRESULT (*GetExtendedFramePosition)(IApplicationView *This, RECT*);
    HRESULT (*GetAppUserModelId)(IApplicationView *This, PWSTR* id);
    HRESULT (*SetAppUserModelId)(IApplicationView *This, PCWSTR id);
    HRESULT (*IsEqualByAppUserModelId)(IApplicationView *This, PCWSTR, int* isEqual);
    HRESULT (*GetViewState)(IApplicationView *This, UINT* pState);
    HRESULT (*SetViewState)(IApplicationView *This, UINT state);
    HRESULT (*GetNeediness)(IApplicationView *This, int* pNeediness);
    HRESULT (*GetLastActivationTimestamp)(IApplicationView *This, ULONGLONG* pTimestamp);
    HRESULT (*SetLastActivationTimestamp)(IApplicationView *This, ULONGLONG timestamp);
    HRESULT (*GetVirtualDesktopId)(IApplicationView *This, GUID* pGuid);
    HRESULT (*SetVirtualDesktopId)(IApplicationView *This, REFGUID guid);
    HRESULT (*GetShowInSwitchers)(IApplicationView *This, int* pShow);
    HRESULT (*SetShowInSwitchers)(IApplicationView *This, int show);
    HRESULT (*GetScaleFactor)(IApplicationView *This, int* pScale);
    HRESULT (*CanReceiveInput)(IApplicationView *This, BOOL* pCanReceive);
    HRESULT (*GetCompatibilityPolicyType)(IApplicationView *This, APPLICATION_VIEW_COMPATIBILITY_POLICY* pPolicy);
    HRESULT (*SetCompatibilityPolicyType)(IApplicationView *This, APPLICATION_VIEW_COMPATIBILITY_POLICY policy);
    HRESULT (*GetSizeConstraints)(IApplicationView *This, IImmersiveMonitor*, SIZE*, SIZE*);
    HRESULT (*GetSizeConstraintsForDpi)(IApplicationView *This, UINT, SIZE*, SIZE*);
    HRESULT (*SetSizeConstraintsForDpi)(IApplicationView *This, const UINT*, const SIZE*, const SIZE*);
    HRESULT (*OnMinSizePreferencesUpdated)(IApplicationView *This, HWND);
    HRESULT (*ApplyOperation)(IApplicationView *This, IApplicationViewOperation*);
    HRESULT (*IsTray)(IApplicationView *This, BOOL* pIsTray);
    HRESULT (*IsInHighZOrderBand)(IApplicationView *This, BOOL* pHighZ);
    HRESULT (*IsSplashScreenPresented)(IApplicationView *This, BOOL* pSplashPresent);
    HRESULT (*Flash)(IApplicationView *This);
    HRESULT (*GetRootSwitchableOwner)(IApplicationView *This, IApplicationView** ppOwner);
    HRESULT (*EnumerateOwnershipTree)(IApplicationView *This, IObjectArray** ppTree);
    HRESULT (*GetEnterpriseId)(IApplicationView *This, PWSTR* id);
    HRESULT (*IsMirrored)(IApplicationView *This, BOOL* pMirrored);
    HRESULT (*Unknown1)(IApplicationView *This, int* pVal);
    HRESULT (*Unknown2)(IApplicationView *This, int* pVal);
    HRESULT (*Unknown3)(IApplicationView *This, int* pVal);
    HRESULT (*Unknown4)(IApplicationView *This, int val);
    HRESULT (*Unknown5)(IApplicationView *This, int* pVal);
    HRESULT (*Unknown6)(IApplicationView *This, int val);
    HRESULT (*Unknown7)(IApplicationView *This);
    HRESULT (*Unknown8)(IApplicationView *This, int* pVal);
    HRESULT (*Unknown9)(IApplicationView *This, int val);
    HRESULT (*Unknown10)(IApplicationView *This, int val1, int val2);
    HRESULT (*Unknown11)(IApplicationView *This, int val);
    HRESULT (*Unknown12)(IApplicationView *This, SIZE* pSize);
} IApplicationViewVtbl;

struct IApplicationView {
    const IApplicationViewVtbl *lpVtbl;
};

#endif 

#ifndef __IAPPVIEWCOLLECTIONCUSTOM_INTERFACE_DEFINED__
#define __IAPPVIEWCOLLECTIONCUSTOM_INTERFACE_DEFINED__

static const IID IID_IAppViewCollectionCustom = 
{ 0x1841C6D7, 0x4F9D, 0x42C0, { 0xAF, 0x41, 0x87, 0x47, 0x53, 0x8F, 0x10, 0xE5 } };

typedef struct IAppViewCollectionCustom IAppViewCollectionCustom;

typedef struct IAppViewCollectionCustomVtbl {
    HRESULT (*QueryInterface)(IAppViewCollectionCustom *This, REFIID riid, LPVOID* ppvObject);
    ULONG   (*AddRef)(IAppViewCollectionCustom *This);
    ULONG   (*Release)(IAppViewCollectionCustom *This);

    HRESULT (*GetViews)(IAppViewCollectionCustom *This, IObjectArray** ppArray);
    HRESULT (*GetViewsByZOrder)(IAppViewCollectionCustom *This, IObjectArray** ppArray);
    HRESULT (*GetViewsByAppUserModelId)(IAppViewCollectionCustom *This, PCWSTR, IObjectArray** ppArray);
    HRESULT (*GetViewForHwnd)(IAppViewCollectionCustom *This, HWND, IApplicationView** ppView);
    HRESULT (*GetViewForApplication)(IAppViewCollectionCustom *This, IImmersiveApplication, IApplicationView**);
    HRESULT (*GetViewForAppUserModelId)(IAppViewCollectionCustom *This, PCWSTR, IApplicationView**);
    HRESULT (*GetViewInFocus)(IAppViewCollectionCustom *This, IApplicationView**);
    HRESULT (*Unknown1)(IAppViewCollectionCustom *This, IApplicationView**);
    HRESULT (*RefreshCollection)(IAppViewCollectionCustom *This);
    HRESULT (*RegisterForApplicationViewChanges)(IAppViewCollectionCustom *This, IApplicationViewChangeListener*, DWORD*);
    HRESULT (*UnregisterForApplicationViewChanges)(IAppViewCollectionCustom *This, DWORD);
} IAppViewCollectionCustomVtbl;

struct IAppViewCollectionCustom {
    const IAppViewCollectionCustomVtbl *lpVtbl;
};

#endif

static const CLSID CLSID_ImmersiveShell = 
{
    0xC2F03A33, 0x21F5, 0x47FA, {0xB4, 0xBB, 0x15, 0x63, 0x62, 0xA2, 0xF2, 0x39}
};

#ifndef __IServiceProvider_INTERFACE_DEFINED__
#define __IServiceProvider_INTERFACE_DEFINED__
DECLARE_INTERFACE_IID_(IServiceProvider, IUnknown, "6D5140C1-7436-11CE-8034-00AA006009FA")
{
    
    STDMETHOD(QueryInterface)(THIS_ REFIID riid, void** ppvObject) PURE;
    STDMETHOD_(ULONG, AddRef)(THIS) PURE;
    STDMETHOD_(ULONG, Release)(THIS) PURE;

    
    STDMETHOD(QueryService)(THIS_ REFGUID guidService, REFIID riid, void** ppvObject) PURE;
};
#endif

HRESULT SetCloakForWindow(HWND hwnd, APPLICATION_VIEW_CLOAK_TYPE cloakType, int cloakFlag);

#endif 
