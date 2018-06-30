# Tempget

Tempget is a cross-platform utility program that downloads files into a
directory structure specified by a "template" file.

## Features

* Create template files to automatically specify what files to retrieve and
  where to place them.
* Selectively extract files from `.zip` (and other) archive files into desired
  locations.
* Template files can be easily generated by another program.
* Human-friendly CLI with progress bars.
* (In the future) Parallel file downloads.
* Cross-platform: works on Windows, Mac, and Linux!

## Usage

First, create a template file. Templates use the
[TOML](https://github.com/toml-lang/toml) configuration format.

### Template file format

The `retrieve` table describes which files to download and where to place them.
For example, the following template will download two files from the internet
and save them to `my_file` and `some_folder/other_file`:

```toml
[retrieve]

# Format is
#     "location_on_disk" = "https://link_to_file"

"my_file" = "https://example.com/"
"some_folder/other_flie" = "https://example.com/some_file"
```

The `extract` table describes how to extract files from an archive, using the
zip archive files as keys. At the moment, each zip file can be handled in one of
two ways

* Extracting all of the contents of the zip file to a folder. The zip file
  should be mapped to the location of the folder to extract to:

  ```toml
  [extract]
  "my_zip_file.zip" = "somewhere/folder_to_extract_to/"
  ```
* Extracting some files to particular locations. The zip file should be mapped
  to a table that maps files in the zip archive to extracted files:
  
  ```toml
  [extract."my_zip_file.zip"]
  "folder_in_zip/file_in_zip" = "somewhere/file_to_extract_to"
  "other_file_in_zip" = "another_file_to_extract_to"
  ```
  
### Running the template download

Once you have created your template file, run

```plain
tempget template.toml
```

where `template.toml` is the name of your template file. You can configure what
`tempget` does by supplying command line flags; see `tempget -h` for more
information.

## Frequently Asked Questions

### Why would I want to use tempget instead of a shell script?

To the best of our knowledge, `tempget` is cross-platform while there is no
built-in cross-platform scripting languages and environments that support the
same functionality. Yes, it might be possible to write a Python (or Ruby, bash,
etc.) script to do what `tempget` does, but Windows does not come with Python
installed.

Additionally, for simple file download tasks, it becomes very tedious to write
scripts to download files to specific locations--especially if some files are
contained in zip files.

### Why would I want to use a shell script instead of tempget?

`tempget` is primary for downloading files to specific locations. Any more
complicated actions like calling commands after downloading can and should be
achieved with your choice of scripting language. If you need to extract all
files of a specific file format from a zip file and then post-process them,
you're probably better off writing a shell script. If you need to download and
compile code source, you're probably better off writing a shell script.

Of course, you can always call `tempget` from a script.

## License

Copyright 2018 Bryan Tan ("Technius")

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
