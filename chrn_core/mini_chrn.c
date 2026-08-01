// gcc mini_chrn.c -o mini -Wall -Wextra
//
// Program that does nothing

#include <stdio.h>
#include <stdlib.h>

#ifdef __unix
#include <dirent.h>

#elif _WIN32

#else

#endif

#define TODO(msg)                                                              \
  printf("TODO: %s\n", msg);                                                   \
  abort();

typedef struct {
} MiniChrn;

MiniChrn new_mini_chrn() {
  MiniChrn mini = {};
  return mini;
}

// Feels like C activates the JAVA MoEs and ints start appearing before my eyes
const int BUFFER_SIZE = 8192;

typedef struct {
  struct dirent **ptr;
  size_t len;
  size_t capacity;
} Entries;

void add_entry(Entries *entries, struct dirent *entry) {
  if (entries->len == entries->capacity) {
    TODO("No")
  }

  struct dirent **to_write = entries->ptr + entries->len;
  *to_write = entry;
  entries->len += 1;
}

void print_entries(Entries *entries) {
  for (size_t i = 0; i < entries->len; ++i) {
    struct dirent *current = *(entries->ptr + i);
    printf("[%zu]: %s\n", i, current->d_name);
  }
}

Entries new_entries(size_t capacity) {
  struct dirent **ptr;
  if (capacity == 0) {
    Entries entries = {.capacity = 0, .len = 0, .ptr = NULL};
    return entries;
  }

  ptr = (struct dirent **)calloc(capacity, sizeof(struct dirent *));
  if (!ptr) {
    perror("Probably fine we're fine");
  }
  Entries entries = {.ptr = ptr, .capacity = capacity, .len = 0};
  return entries;
}

int main() {
  // No because why did this instance where the code isn't even abstracted to be
  // OS agnostic take so long? How are the mini chrns to grow?
  DIR *dir;

  if ((dir = opendir(".")) == NULL) {
    // mhm
    fprintf(stderr, "Coould not open cccurrent chrn");
    return 1;
  }

  Entries entries = new_entries(14);
  struct dirent *entry;
  while ((entry = readdir(dir)) != NULL) {
    add_entry(&entries, entry);
  }
  print_entries(&entries);

  MiniChrn mini = new_mini_chrn();
}
