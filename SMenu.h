#define BUF_LEN 4096

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
    int (*itemsAction)(CHAR** items);
} MenuDefinition;

struct NamedCommand
{
    char* name;
    char* expression;
    int expressionLen;
    BOOL hasReplacement;
    int indexOfReplacement;
    NamedCommand *next;
};

struct MenuKeyBinding
{
    unsigned int modifier;
    unsigned int key;
    NamedCommand* command;
    MenuKeyBinding* next;
    BOOL reloadAfter;
    BOOL isLoadCommand;
    BOOL quitAfter;
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
    Item *next;
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
    ItemsView *itemsView;
    MenuKeyBinding *keyBindings;
    MenuKeyBinding *lastKeyBinding;
    void (*onEscape)(void);
    BOOL cancelSearch;
    HANDLE cancelSearchEvent;
    CRITICAL_SECTION cancelSearchCriticalSection; 
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
} ProcessChunkJob;

struct ItemsView
{
    SearchView *searchView;
    HWND hwnd;
    HWND headerHwnd;
    HWND summaryHwnd;
    int height;
    int width;
    BOOL hasHeader;
    BOOL headerSet;
    BOOL hasLoadCommand;
    NamedCommand *loadCommand;
    int maxDisplayItems;
    int numberOfItemsMatched;
    BOOL isReading;
    BOOL isSearching;
    fzf_slab_t *fzfSlab;
    fzf_pattern_t *fzfPattern;
    UINT numberOfItems;
    Chunk *chunks;
    NamedCommand *namedCommands;
    NamedCommand *lastNamedCommand;
    char* selectedString;
    BOOL hasReturnRange;
    int returnRangeStart;
    int returnRangeEnd;
    void (*onSelection)(char* stdOut);
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

void MenuDefinition_ParseAndAddKeyBinding(MenuDefinition *self, char *argText, BOOL isLoadCommand);
void MenuDefinition_AddNamedCommand(MenuDefinition *self, char *argText);
MenuView *menu_create(int left, int top, int width, int height, TCHAR *title);
void menu_run_definition(MenuView *self, MenuDefinition *menuDefinition);
void MenuDefinition_ParseAndAddLoadCommand(MenuDefinition *self, char *argText);
void MenuDefinition_ParseAndSetRange(MenuDefinition *self, char *argText);