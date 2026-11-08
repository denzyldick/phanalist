<?php
// Safe: validated upload destination
move_uploaded_file($_FILES['file']['tmp_name'], '/var/www/uploads/file.txt');
