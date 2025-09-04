#include "nfm_menu.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Callback functions
void on_array_select(char* selected_line, void* state) {
    printf("Selected: %s\n", selected_line);
}

void on_array_closed(void) {
    printf("Array columns menu closed\n");
}

int main() {
    // Load the NFM library
    HMODULE hModule = nfm_load_library("libnfm.dll");
    if (!hModule) {
        return 1;
    }

    // Example 1: Employee data with column names
    printf("Example 1: Employee data with headers and preview\n");
    
    // Create sample data (3 rows x 4 columns)
    char* employee_data[3][4] = {
        {"John", "Doe", "30", "Engineer"},
        {"Jane", "Smith", "25", "Designer"},
        {"Bob", "Johnson", "35", "Manager"}
    };
    
    // Convert to the format expected by the native bridge
    char** data_ptrs[3];
    for (int i = 0; i < 3; i++) {
        data_ptrs[i] = employee_data[i];
    }
    
    // Column names
    char* column_names[] = {"First Name", "Last Name", "Age", "Role"};
    
    // Call the native bridge function
    nfm_show_array_columns(
        (char***)data_ptrs,       // arrayData
        3,                        // rowCount
        4,                        // columnCount
        column_names,             // columnNames
        4,                        // columnNamesCount
        1,                        // showPreview (enabled)
        on_array_select,          // onSelect callback
        on_array_closed,          // onClosed callback
        NULL                      // state
    );

    printf("Press Enter to try another example...\n");
    getchar();

    // Example 2: File data without column names
    printf("\nExample 2: File data without headers, no preview\n");
    
    char* file_data[4][3] = {
        {"document.pdf", "2048", "2023-01-15"},
        {"image.png", "1024", "2023-02-20"},
        {"spreadsheet.xlsx", "4096", "2023-03-10"},
        {"presentation.pptx", "3072", "2023-04-05"}
    };
    
    char** file_data_ptrs[4];
    for (int i = 0; i < 4; i++) {
        file_data_ptrs[i] = file_data[i];
    }
    
    nfm_show_array_columns(
        (char***)file_data_ptrs,  // arrayData
        4,                        // rowCount
        3,                        // columnCount
        NULL,                     // columnNames (none)
        0,                        // columnNamesCount
        0,                        // showPreview (disabled)
        on_array_select,          // onSelect callback
        on_array_closed,          // onClosed callback
        NULL                      // state
    );

    // Cleanup
    nfm_unload_library(hModule);
    return 0;
}