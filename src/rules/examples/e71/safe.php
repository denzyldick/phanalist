<?php
// Safe: upload to a fixed, validated directory
move_uploaded_file($_FILES['file']['tmp_name'], '/var/www/uploads/file.txt');
