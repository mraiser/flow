var me = this; 
var ME = $('#'+me.UUID)[0];

me.ready = function(){
  var el = $(ME).find("div");
  send_plus(299,121, function(result) {
    el.append("test_plus: "+JSON.stringify(result)+"<BR>");
    send_plus("299",121, function(result) {
      el.append("test_plus: "+JSON.stringify(result)+"<BR>");
      send_plus({"a":299},[121], function(result) {
        el.append("test_plus: "+JSON.stringify(result)+"<BR>");
      });
    });
  });
};
