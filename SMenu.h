#define BUF_LEN 4096

enum Mode
{
    Normal = 1,
    Help = 2,
};

typedef struct MenuKeyBinding MenuKeyBinding;
typedef struct NamedCommand NamedCommand;

typedef struct MenuDefinition
{
    MenuKeyBinding *keyBindings;
    MenuKeyBinding *lastKeyBinding;
    NamedCommand *namedCommands;
    NamedCommand *lastNamedCommand;
    BOOL hasLoadCommand;
    BOOL hasHeader;
    NamedCommand *loadCommand;
    int returnRangeStart;
    int returnRangeEnd;
    BOOL hasReturnRange;
    void (*onSelection)(char* stdOut);
    void (*onEscape)(void);
    int (*itemsAction)(int maxItems, CHAR** items);
} MenuDefinition;

struct NamedCommand
{
    char* name;
    char* expression;
    void (*action)(char* stdOut);
    void (*action2)(void* state);
    void* action2State;
    int expressionLen;
    BOOL trimEnd;
    BOOL hasReplacement;
    int indexOfReplacement;
    NamedCommand *next;
    BOOL reloadAfter;
    BOOL quitAfter;
    BOOL hasTextRange;
    int textRangeStart;
    int textRangeEnd;
};

struct MenuKeyBinding
{
    unsigned int modifier;
    unsigned int key;
    NamedCommand* command;
    MenuKeyBinding* next;
    BOOL isLoadCommand;
    int (*loadAction)(int maxItems, CHAR** items);
    char *loadActionDescription;
};

typedef struct ItemsView ItemsView;
typedef struct ProcessAsyncState ProcessAsyncState;
struct ProcessAsyncState
{
    ItemsView *itemsView;
    HANDLE waitHandle;
    HANDLE readFileHandle;
    HANDLE processId;
    HANDLE thread;
    BOOL reloadAfter;
    BOOL quitAfter;
};

typedef struct Item Item;
struct Item
{
    CHAR *text;
};

typedef struct ItemMatchResult
{
    Item *item;
    int score;
} ItemMatchResult;

typedef struct Chunk Chunk;
struct Chunk
{
    Item **items;
    int count;
    Chunk *next;
};

typedef struct DisplayItemList
{
    int maxItems;
    int count;
    int minScore;
    ItemMatchResult **items;
} DisplayItemList;

typedef struct SearchView
{
    HWND hwnd;
    enum Mode mode;
    HWND helpHwnd;
    HWND helpHeaderHwnd;
    CHAR *searchString;
    ItemsView *itemsView;
    MenuKeyBinding *keyBindings;
    MenuKeyBinding *lastKeyBinding;
    void (*onEscape)(void);
    BOOL isSearching;
    BOOL cancelSearch;
    HANDLE searchEvent;
} SearchView;

typedef struct ProcessChunkJob
{
    Chunk *chunk;
    DisplayItemList *displayItemList;
    ItemMatchResult **allItemsWithScores;
    int numberOfMatches;
    fzf_pattern_t *fzfPattern;
    fzf_slab_t *fzfSlab;
    int jobNumber;
    char* searchString;
    SearchView *searchView;
    UINT currentSearchNumber;
} ProcessChunkJob;

struct ItemsView
{
    SearchView *searchView;
    HWND hwnd;
    HWND headerHwnd;
    HWND summaryHwnd;
    HWND cmdResultHwnd;
    int height;
    int width;
    BOOL hasHeader;
    BOOL headerSet;
    BOOL hasLoadCommand;
    NamedCommand *loadCommand;
    int (*loadAction)(int maxItems, CHAR**);
    int maxDisplayItems;
    int numberOfItemsMatched;
    BOOL isReading;
    BOOL isSearching;
    fzf_slab_t *fzfSlab;
    UINT numberOfItems;
    Chunk *chunks;
    NamedCommand *namedCommands;
    NamedCommand *lastNamedCommand;
    char selectedString[BUF_LEN];
    BOOL itemSelected;
    BOOL hasReturnRange;
    int returnRangeStart;
    int returnRangeEnd;
    void (*onSelection)(char* stdOut);
    BOOL isLoading;
    BOOL cancelLoad;
    HANDLE loadEvent;
    CRITICAL_SECTION loadCriticalSection;
    Item *displayItems[1024];
    int numberOfDisplayItems;
};

typedef struct ProcessCmdOutputJob
{
    HANDLE readHandle;
    ItemsView *itemsView;
} ProcessCmdOutputJob;

typedef struct MenuView
{
    HWND hwnd;
    int height;
    int width;
    SearchView *searchView;
    ItemsView *itemsView;
} MenuView;

void MenuDefinition_AddKeyBindingToNamedCommand(MenuDefinition *self, NamedCommand *namedCommand, unsigned int modifier, unsigned int key, BOOL isLoadCommand);
void MenuDefinition_ParseAndAddKeyBinding(MenuDefinition *self, char *argText, BOOL isLoadCommand);
NamedCommand *MenuDefinition_AddNamedCommand(MenuDefinition *self, char *argText, BOOL reloadAfter, BOOL quitAfter);
void NamedCommand_SetTextRange(NamedCommand *self, int start, int end, BOOL trimEnd);
NamedCommand* MenuDefinition_AddActionNamedCommand_WithTextRange(MenuDefinition *self, CHAR *nameBuff, void (*action)(CHAR *text), BOOL reloadAfter, BOOL quitAfter);
MenuView *menu_create_with_size(int left, int top, int width, int height, TCHAR *title);
MenuView *menu_create(TCHAR *title);
void menu_run_definition(MenuView *self, MenuDefinition *menuDefinition);
void MenuDefinition_ParseAndAddLoadCommand(MenuDefinition *self, char *argText);
void MenuDefinition_ParseAndSetRange(MenuDefinition *self, char *argText);
void MenuDefinition_AddLoadActionKeyBinding(MenuDefinition *self, unsigned int modifier, unsigned int key, int (*loadAction)(int maxItems, CHAR**), char* loadActionDescription);
void menu_definition_set_load_action(MenuDefinition *self, int (*loadAction)(int maxItems, CHAR** items));
void menu_definition_set_load_command(MenuDefinition *self, NamedCommand *loadCommand);
MenuDefinition* menu_definition_create(MenuView *menuView);
