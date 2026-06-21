APOLLO v0.1.0

I've been reorganizing my music library lately and I've had some trouble sorting through all my files, some of which were misnamed. That's why a few months ago I made a small python script to help me sort them. However, that too soon buckled under the load of files it had to handle, and was taking a substantial amount of time to execute each time. Therefore, to save roughly 20 seconds once per month, I efficiently decided to invest 10 hours of my time to make this hyper optimized file sorter that does the exact same thing but 20x faster.

This program will traverse the given input directory, read the metadata of all music files (files with .mp3, .ogg, or .wav suffixes) (and try to guess missing fields) and try to sort them according to Artist/Album/Track in the given output directory. It can also create .m3u playlist file in the Playlists/ folder, and can process batches of playlist (a folder full of folders, each of which represents a playlist).

According to my very sophisticated benchmarks ('time apollo') I determined the userspace runtime to be approximately 20 microseconds per file, with filesystem requests being delayed and batched by multiple threads to minimize real runtime.
Also the design is very modular and I will most likely implement other features for more sophisticated sorting very soon.


Feel free to use this for your own music library, or any project you are working on (credit would be appreciated), and if you have any questions, suggestions or remarks, don't hesitate to contact me!

TODO:
Custom sorting patterns (ideally custom pattern)
Further reduction in effective syscalls (breaking down paths into trees to not 'check/create' directories that are guaranteed to exist by then)
Slightly improved logging
Automatic file culling
(long-term) Possibly filters for modifying metadata of written files (e.g. deleting faulty entries or conflicting metadata)