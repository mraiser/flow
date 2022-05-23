var me = this; 
var ME = $('#'+me.UUID)[0];

me.ready = function(){
  var el = $(ME).find("div");
  send_test_add(299,121, function(result) {
    el.append("test_add: "+JSON.stringify(result)+"<BR>");
    send_test_command(210, function(result) {
      el.append("test_command: "+JSON.stringify(result)+"<BR>");
      send_test_conditionals(true, function(result) {
        el.append("test_conditionals:"+JSON.stringify(result)+"<BR>");
        send_test_lists([1,2,3], function(result) {
          el.append("test_lists:"+JSON.stringify(result)+"<BR>");
          send_test_loop(0, function(result) {
            el.append("test_loop:"+JSON.stringify(result)+"<BR>");
            send_test_speed(100000, function(result) {
              el.append("test_speed:"+JSON.stringify(result)+"<BR>");
              send_test_rust([400],[20], function(result) {
                el.append("test_rust: "+JSON.stringify(result)+"<BR>");
              });
            });
          });
        });
      });
    });
  });
};