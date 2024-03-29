#include "CommonTypes.h"

typedef struct WindowManagerState WindowManagerState;
typedef struct Workspace Workspace;
typedef struct WorkspaceFilterData WorkspaceFilterData;
typedef struct Client Client;
typedef struct ClientData ClientData;
typedef struct Monitor Monitor;
typedef struct Layout Layout;
typedef struct Bar Bar;
typedef struct Button Button;
typedef struct Command Command;
typedef struct KeyBinding KeyBinding;
typedef struct ScratchWindow ScratchWindow;
typedef struct Configuration Configuration;
typedef struct BarSegment BarSegment;
typedef struct BarSegmentConfiguration BarSegmentConfiguration;

typedef BOOL (*WindowFilter)(Client *client);
typedef BOOL (*ScratchFilter)(ScratchWindow *self, Client *client);

#define MAX_COMMANDS 256

#define VK_A 0x41
#define VK_B 0x42
#define VK_C 0x43
#define VK_D 0x44
#define VK_E 0x45
#define VK_F 0x46
#define VK_G 0x47
#define VK_H 0x48
#define VK_I 0x49
#define VK_J 0x4A
#define VK_K 0x4B
#define VK_L 0x4C
#define VK_M 0x4D
#define VK_N 0x4E
#define VK_O 0x4F
#define VK_P 0x50
#define VK_Q 0x51
#define VK_R 0x52
#define VK_S 0x53
#define VK_T 0x54
#define VK_U 0x55
#define VK_V 0x56
#define VK_W 0x57
#define VK_X 0x58
#define VK_Y 0x59
#define VK_Z 0x5A

#define VK_0 0x30
#define VK_1 0x31
#define VK_2 0x32
#define VK_3 0x33
#define VK_4 0x34
#define VK_5 0x35
#define VK_6 0x36
#define VK_7 0x37
#define VK_8 0x38
#define VK_9 0x39

enum Modifiers
{
    LShift = 0x1,
    RShift = 0x2,
    LAlt = 0x4,
    RAlt = 0x8,
    LWin = 0x10,
    RWin = 0x20,
    RCtl = 0x40,
    LCtl = 0x80
};

enum WindowRoutingMode
{
    FilteredAndRoutedToWorkspace = 0x1,
    FilteredCurrentWorkspace = 0x2,
    NotFilteredCurrentWorkspace = 0x4,
    FilteredRoutedNonFilteredCurrentWorkspace = 0x8
};

typedef struct BarSegmentHeader
{
    TCHAR text[MAX_PATH];
    size_t textLength;
    bool isIcon;
    RECT rect;
} BarSegmentHeader;

typedef struct WorkspaceStyle
{
    int gapWidth;
    int scratchWindowsScreenPadding;
    COLORREF dropTargetColor;
    HBRUSH _dropTargetBrush;
} WorkspaceStyle;

struct BarSegmentConfiguration
{
    BarSegmentHeader *header;
    BarSegmentHeader *separator;
    BarSegmentHeader *variable;
    int variableTextFixedWidth;
    void (*variableTextFunc) (TCHAR *toFill, int maxLen);
};

struct Configuration
{
    BOOL (*windowsThatShouldNotFloatFunc) (Client *client, LONG_PTR styles, LONG_PTR exStyles);
    BOOL (*shouldAlwaysExcludeFunc) (Client* client);
    BOOL (*useOldMoveLogicFunc) (Client *client);
    BOOL (*clientShouldUseMinimizeToHide) (Client *client);
    long barHeight;
    BarSegmentConfiguration **barSegments;
    int numberOfBarSegments;
    Monitor **monitors;
    Workspace **workspaces;
    enum WindowRoutingMode windowRoutingMode;
    BOOL alwaysRedraw;
    int nonFloatWindowHeightMinimum;
    BOOL floatUwpWindows;
    int easyResizeModifiers;
    int dragDropFloatModifier;
    int floatWindowMovement;
    int borderWindowBackgroundTransparency;
    TextStyle *textStyle;
    WorkspaceStyle *workspaceStyle;
};

struct WorkspaceFilterData
{
    TCHAR **titles;
    int numberOfTitles;
    TCHAR **notTitles;
    int numberOfNotTitles;
    TCHAR **processImageNames;
    int numberOfProcessImageNames;
    TCHAR **notProcessImageNames;
    int numberOfNotProcessImageNames;
    TCHAR **classNames;
    int numberOfClassNames;
    TCHAR **notClassNames;
    int numberOfNotClassNames;
};

struct Workspace
{
    TCHAR* name;
    Client *clients;
    Client *minimizedClients;
    Monitor *monitor;
    Client *selected;
    WindowFilter windowFilter;
    HWND barButtonHwnd;
    WCHAR *tag;
    bool isIcon;
    Button **buttons;
    int numberOfButtons;
    int mainOffset;
    Layout *layout;
    Client *lastClient;
    int numberOfClients;
    WorkspaceFilterData *filterData;
};

struct Client
{
    BOOL isVisible;
    ClientData *data;
    Client *next;
    Client *previous;
    Workspace *workspace;
};

struct ScratchWindow
{
    CHAR *name;
    TCHAR *processImageName;
    CHAR *cmd;
    CHAR *cmdArgs;
    TCHAR *uniqueStr;
    Client *client;
    void (*stdOutCallback) (CHAR *);
    WindowFilter windowFilter;
    ScratchFilter scratchFilter;
    ScratchWindow *next;
    void (*runFunc) (ScratchWindow *, Monitor *monitor, int);
    void (*beforeCmd) (ScratchWindow *, WindowManagerState *, CHAR *, size_t);
    ULONGLONG timeout;
};

struct ClientData
{
    int x, y, w, h;
    BOOL isDirty;
    HWND hwnd;
    DWORD processId;
    TCHAR *processImageName;
    TCHAR *className;
    TCHAR *title;
    TCHAR *commandLine;
    BOOL isElevated;
    BOOL isMinimized;
    BOOL isScratchWindow;
    BOOL isScratchWindowBoundToWorkspace;
    BOOL useMinimizeToHide;
};

struct Monitor
{
    int id;
    int top;
    int bottom;
    int xOffset;
    int h;
    int w;
    Workspace *workspace;
    ScratchWindow *scratchWindow;
    BOOL isHidden;
    HWND barHwnd;
    BOOL selected;
    Bar *bar;
    Monitor *next;
    WorkspaceStyle *workspaceStyle;
};

struct Layout
{
    TCHAR *tag;
    void (*select_next_window) (Workspace *workspace);
    void (*select_previous_window) (Workspace *workspace);
    void (*swap_clients) (Client *client1, Client *client2);
    void (*move_client_to_main) (Client *client);
    void (*move_client_next) (Client *client);
    void (*move_client_previous) (Client *client);
    void (*apply_to_workspace) (Workspace *workspace);
    Layout *next;
};

struct Button
{
    HWND hwnd;
    Workspace *workspace;
    Bar *bar;
    RECT *rect;
    BOOL isSelected;
    BOOL hasClients;
};

struct Bar
{
    HWND hwnd;
    Monitor *monitor;
    int numberOfButtons;
    Button **buttons;
    RECT *selectedWindowDescRect;
    RECT *timesRect;
    BarSegment **segments;
    int numberOfSegments;
    WindowManagerState *windowManager;
};

struct BarSegment
{
    BarSegmentHeader *header; 
    BarSegmentHeader *separator; 
    BarSegmentHeader *variable;
    void (*variableTextFunc) (TCHAR *toFill, int maxLen);
};

struct Command
{
    CHAR *name;
    CHAR *type;
    void (*execute)(Command *self);
    void (*getDescription)(Command *self, int maxLen, CHAR *toFill);
    void (*action)(WindowManagerState *windowManager);
    void (*monitorAction)(WindowManagerState *windowManager, Monitor *arg);
    Monitor *monitorArg;
    void (*workspaceAction)(WindowManagerState *windowManager, Workspace *arg);
    Workspace *workspaceArg;
    void (*scratchWindowAction)(WindowManagerState *windowManager, ScratchWindow *arg);
    ScratchWindow *scratchWindowArg;
    void (*menuAction)(MenuDefinition *arg);
    MenuDefinition *menuArg;
    void (*shellAction) (TCHAR *arg);
    TCHAR *shellArg;
    KeyBinding *keyBinding;
    WindowManagerState *windowManager;
};

struct KeyBinding
{
    Command *command;
    CHAR *name;
    int modifiers;
    unsigned int key;
    KeyBinding *next;
};

struct WindowManagerState
{
    Monitor *primaryMonitor;
    Monitor *secondaryMonitor;
    Monitor *hiddenWindowMonitor;
    Monitor *selectedMonitor;
    HWND borderWindowHwnd;
    Monitor **monitors;
    int numberOfMonitors;
    int numberOfDisplayMonitors;
    Workspace **workspaces;
    int numberOfWorkspaces;
    Workspace *lastWorkspace;
    KeyBinding *keyBindings;
    ScratchWindow *scratchWindows;
    MenuView *menuView;
    BOOL menuVisible;
    size_t longestCommandName;
    Command *commands[MAX_COMMANDS];
    int numberOfCommands;
    BOOL isForegroundWindowSameAsSelectMonitorSelected;
    HWND eventForegroundHwnd;
    int floatWindowMovement;
    enum WindowRoutingMode currentWindowRoutingMode;
    BOOL (*useOldMoveLogicFunc) (Client *client);
    TextStyle *textStyle;
};

typedef struct DragDropState
{
    bool inProgress;
    HWND dragHwnd;
    HWND dropTargetHwnd;
    WindowManagerState *windowManager;
} DragDropState;

typedef struct ResizeState
{
    bool easyResizeInProgress;
    POINT easyResizeStartPoint;
    int easyResizeStartOffset;
    bool regularResizeInProgress;
    Client *regularResizeClient;
    WindowManagerState *windowManager;
} ResizeState;

extern Layout deckLayout;
extern Layout monacleLayout;
extern Layout tileLayout;

extern TCHAR *scratchWindowTitle;

void configure(Configuration *configuration);
HFONT initalize_font(LPCWSTR fontName, int size);
Workspace* workspace_register(TCHAR *name, WCHAR* tag, bool isIcon, Layout *layout);
Workspace* workspace_register_with_window_filter(TCHAR *name, WindowFilter windowFilter, WCHAR* tag, bool isIcon, Layout *layout);
void workspace_register_processimagename_contains_filter(Workspace *workspace, TCHAR *className);
void workspace_register_classname_contains_filter(Workspace *workspace, TCHAR *className);
void workspace_register_title_contains_filter(Workspace *workspace, TCHAR *title);
void workspace_register_title_not_contains_filter(Workspace *workspace, TCHAR *title);
void workspace_focus_selected_window(WindowManagerState *windowManagerState, Workspace *workspace);
Workspace* workspace_create(TCHAR *name, WindowFilter windowFilter, WCHAR* tag, Layout *layout, int numberOfButtons);

void keybinding_create_with_no_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (WindowManagerState*));
void keybinding_create_with_workspace_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (WindowManagerState*, Workspace*), Workspace *arg);
void keybinding_create_with_scratchwindow_arg(CHAR *name, int modifiers, unsigned int key, ScratchWindow *arg);
void keybinding_create_with_menu_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (MenuDefinition*), MenuDefinition *arg);
void keybinding_create_with_shell_arg(CHAR *name, int modifiers, unsigned int key, void (*action) (TCHAR*), TCHAR *arg);

TCHAR* client_get_command_line(Client *self);

void configuration_register_default_text_style(Configuration *self, TCHAR *fontName, int normalFontSize, int iconFontSize);
void configuration_add_bar_segment(
        Configuration *self,
        TCHAR *separatorText,
        bool separatorIsIcon,
        int variableTextFixedWidth,
        bool variableIsIcon,
        void (*variableTextFunc)(TCHAR *toFill, int maxLen));
void configuration_add_bar_segment_with_header(
        Configuration *self,
        TCHAR *separatorText,
        bool separatorIsIcon,
        TCHAR *headerText,
        bool headerIsIcon,
        int variableTextFixedWidth,
        bool variableIsIcon,
        void (*variableTextFunc)(TCHAR *toFill, int maxLen));
void fill_cpu(TCHAR *toFill, int maxLen);
void fill_volume_percent(TCHAR *toFill, int maxLen);
void fill_system_time(TCHAR *toFill, int maxLen);
void fill_local_time(TCHAR *toFill, int maxLen);
void fill_local_date(TCHAR *toFill, int maxLen);
void fill_memory_percent(TCHAR *toFill, int maxLen);
void fill_is_connected_to_internet(TCHAR *toFill, int maxLen);

void goto_last_workspace(WindowManagerState *self);
void windowManager_move_workspace_to_monitor(WindowManagerState *windowManagerState, Monitor *monitor, Workspace *workspace);
void redraw_focused_window(WindowManagerState *self);
void select_next_window(WindowManagerState *self);
void select_previous_window(WindowManagerState *self);
void monitor_select_next(WindowManagerState *self);
void start_launcher(CHAR *cmdArgs);
void start_scratch_not_elevated(CHAR *cmdArgs);
void scratch_terminal_register_with_unique_string(CHAR *cmd, int modifiers, int key, TCHAR *uniqueStr);
void scratch_terminal_register(CHAR *cmd, int modifiers, int key, TCHAR *uniqueStr, ScratchFilter scratchFilter);
ScratchWindow *register_scratch_with_unique_string(TCHAR *processImageName, CHAR *name, char *cmd, TCHAR *uniqueStr);
ScratchWindow *register_windows_terminal_scratch_with_unique_string(CHAR *name, char *cmd, TCHAR *uniqueStr);
void process_with_stdout_start(CHAR *cmdArgs, void (*onSuccess) (CHAR *));
void start_app(TCHAR *processExe);
void start_app_non_elevated(TCHAR *processExe);
void toggle_selected_monitor_layout(WindowManagerState *self);
void arrange_clients_in_selected_workspace(WindowManagerState *self);
void workspace_increase_main_width_selected_monitor(WindowManagerState *self);
void workspace_decrease_main_width_selected_monitor(WindowManagerState *self);
void move_focused_window_to_main(WindowManagerState *self);
void move_secondary_monitor_focused_window_to_main(WindowManagerState *self);
void move_focused_window_left(WindowManagerState *self);
void move_focused_window_right(WindowManagerState *self);
void move_focused_window_up(WindowManagerState *self);
void move_focused_window_down(WindowManagerState *self);
void swap_selected_monitor_to(WindowManagerState* self, Workspace *workspace);
void move_focused_window_to_workspace(WindowManagerState *self, Workspace *workspace);
void move_focused_window_to_selected_monitor_workspace(WindowManagerState *self);
void close_focused_window(WindowManagerState *self);
void kill_focused_window(WindowManagerState *self);
void float_window_move_right(WindowManagerState *self, HWND hwnd);
void float_window_move_left(WindowManagerState *self, HWND hwnd);
void float_window_move_up(WindowManagerState *self, HWND hwnd);
void float_window_move_down(WindowManagerState *self, HWND hwnd);
void taskbar_toggle(WindowManagerState *self);
void mimimize_focused_window(WindowManagerState *self);
void move_focused_client_next(WindowManagerState *self);
void move_focused_client_previous(WindowManagerState *self);
void swap_selected_monitor_to_deck_layout(WindowManagerState *self);
void swap_selected_monitor_to_monacle_layout(WindowManagerState *self);
void swap_selected_monitor_to_tile_layout(WindowManagerState *self);
void toggle_create_window_in_current_workspace(WindowManagerState *self);
void toggle_ignore_workspace_filters(WindowManagerState *self);
void client_stop_managing(WindowManagerState *self);
void open_windows_scratch_start(void);
void open_program_scratch_start(void);
void open_program_scratch_start_not_elevated(void);
void open_windows_scratch_exit_callback(char *stdOut, void *state);
void open_program_scratch_callback_not_elevated(char *stdOut, void *state);
void open_program_scratch_callback(char *stdOut, void *state);
void open_process_list_scratch_callback(char *stdOut);
void open_process_list(void);
void quit(WindowManagerState *self);

void menu_defintion_register(MenuDefinition *definition);
void menu_on_escape(void *state);

MenuDefinition* menu_create_and_register(void);
void menu_run(MenuDefinition *definition);
void show_clients(void);
void show_keybindings(ScratchWindow *self, Monitor *monitor, int scratchWindowsScreenPadding);
void keybindings_register_defaults(void);
void keybindings_register_defaults_with_modifiers(int modifiers);
void keybindings_register_float_window_movements(int modifiers);
void register_keybindings_menu_with_modifiers(int modifiers, int virtualKey);
void register_keybindings_menu(void);
void register_list_processes_menu(int modifers, int virtualKey);
void register_list_windows_memu(int modifers, int virtualKey);
void register_list_services_menu(int modifiers, int virtualKey);
void register_program_launcher_menu(int modifiers, int virtualKey, CHAR** directories, size_t numberOfDirectories, BOOL isElevated);
void register_secondary_monitor_default_bindings(Monitor *pMonitor, Monitor *sMonitor, Workspace **spaces);
void register_secondary_monitor_default_bindings_with_modifiers(int modifiers, Monitor *pMonitor, Monitor *sMonitor, Workspace **spaces);
BOOL should_use_old_move_logic(Client* client);
BOOL is_float_window(Client *client, LONG_PTR styles, LONG_PTR exStyles);
HFONT initalize_font();
