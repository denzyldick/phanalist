<?php
// Bad: NoSQL injection via user-controlled input
$filter = $_GET['filter'] ?? '';
$collection->insertOne($filter);
$result = mongo_find("users", "name = " . $_POST['name']);
